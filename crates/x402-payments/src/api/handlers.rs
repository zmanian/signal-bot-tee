//! HTTP API handlers.

use super::types::*;
use crate::config::PaymentConfig;
use crate::credits::{CreditStore, PricingCalculator};
use crate::error::PaymentError;
use crate::types::{Chain, Deposit};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use tracing::{error, info};

/// Shared application state for handlers.
pub struct AppState {
    pub credit_store: Arc<CreditStore>,
    pub config: PaymentConfig,
    pub pricing: PricingCalculator,
    // TODO: Add chain facilitators when implemented
    // pub base: Option<Arc<BaseFacilitator>>,
    // pub near: Option<Arc<NearFacilitator>>,
    // pub solana: Option<Arc<SolanaFacilitator>>,
}

impl AppState {
    pub fn new(
        credit_store: Arc<CreditStore>,
        config: PaymentConfig,
    ) -> Self {
        let pricing = PricingCalculator::new(config.pricing.clone());
        Self {
            credit_store,
            config,
            pricing,
        }
    }
}

/// Create the payment API router.
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/v1/balance/:user_id", get(get_balance))
        .route("/v1/deposits/:user_id", get(get_deposits))
        .route("/v1/deposit", post(process_deposit))
        .route("/v1/deposit-address/:chain", get(get_deposit_address))
        .route("/v1/pricing", get(get_pricing))
        .with_state(state)
}

/// Health check endpoint.
async fn health_check(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let chains = vec![
        ChainHealth {
            chain: Chain::Base,
            enabled: state.config.base.as_ref().is_some_and(|c| c.enabled),
            healthy: true, // TODO: Check actual health
        },
        ChainHealth {
            chain: Chain::Near,
            enabled: state.config.near.as_ref().is_some_and(|c| c.enabled),
            healthy: true,
        },
        ChainHealth {
            chain: Chain::Solana,
            enabled: state.config.solana.as_ref().is_some_and(|c| c.enabled),
            healthy: true,
        },
    ];

    Json(HealthResponse {
        healthy: true,
        payments_enabled: state.config.enabled,
        chains,
    })
}

/// Get credit balance for a user.
async fn get_balance(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
) -> Result<Json<BalanceResponse>, (StatusCode, Json<ErrorResponse>)> {
    let balance = state.credit_store.get_balance(&user_id).await;

    Ok(Json(BalanceResponse {
        user_id: balance.user_id,
        credits_remaining: balance.credits_remaining,
        credits_remaining_usdc: PricingCalculator::format_usdc(balance.credits_remaining),
        total_deposited_usdc: PricingCalculator::format_usdc(balance.total_deposited),
        total_consumed_usdc: PricingCalculator::format_usdc(balance.total_consumed),
    }))
}

/// Get deposits for a user.
async fn get_deposits(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
) -> Json<Vec<Deposit>> {
    let deposits = state.credit_store.get_deposits(&user_id).await;
    Json(deposits)
}

/// Process a deposit.
async fn process_deposit(
    State(state): State<Arc<AppState>>,
    Json(request): Json<DepositRequest>,
) -> Result<Json<DepositResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check if chain is enabled
    let chain_enabled = match request.chain {
        Chain::Base => state.config.base.as_ref().is_some_and(|c| c.enabled),
        Chain::Near => state.config.near.as_ref().is_some_and(|c| c.enabled),
        Chain::Solana => state.config.solana.as_ref().is_some_and(|c| c.enabled),
    };

    if !chain_enabled {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                format!("Chain {} is not enabled", request.chain),
                "CHAIN_DISABLED",
            )),
        ));
    }

    // Check for duplicate transaction
    if state.credit_store.is_tx_processed(&request.tx_hash).await {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse::new(
                "Transaction already processed",
                "DUPLICATE_TX",
            )),
        ));
    }

    // TODO: Verify payment on-chain
    // let verification = facilitator.verify_payment(&payload).await?;
    // if !verification.valid {
    //     return Err(...);
    // }

    // For now, trust the claimed amount (will be verified when chain support is added)
    let credits = state.pricing.usdc_to_credits(request.amount);

    let deposit = Deposit::new_pending(
        request.user_id.clone(),
        request.chain,
        request.tx_hash.clone(),
        request.amount,
        credits,
    );

    let deposit_id = deposit.id.clone();
    let tx_hash = deposit.tx_hash.clone();

    match state.credit_store.add_credits(deposit).await {
        Ok(balance) => {
            info!(
                "Processed deposit for {}: {} credits",
                request.user_id, credits
            );

            Ok(Json(DepositResponse {
                deposit_id,
                credits_granted: credits,
                new_balance: balance.credits_remaining,
                tx_hash,
                status: crate::types::DepositStatus::Confirmed,
            }))
        }
        Err(PaymentError::DuplicateTransaction(_)) => Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse::new(
                "Transaction already processed",
                "DUPLICATE_TX",
            )),
        )),
        Err(e) => {
            error!("Failed to process deposit: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(e.to_string(), "INTERNAL_ERROR")),
            ))
        }
    }
}

/// Get deposit address for a chain.
async fn get_deposit_address(
    State(state): State<Arc<AppState>>,
    Path(chain): Path<String>,
) -> Result<Json<DepositAddressResponse>, (StatusCode, Json<ErrorResponse>)> {
    let chain = match chain.to_lowercase().as_str() {
        "base" => Chain::Base,
        "near" => Chain::Near,
        "solana" => Chain::Solana,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    format!("Unknown chain: {}", chain),
                    "UNKNOWN_CHAIN",
                )),
            ));
        }
    };

    // TODO: Get actual deposit addresses from facilitators
    let (address, token_contract) = match chain {
        Chain::Base => {
            let config = state.config.base.as_ref().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse::new("Base not configured", "CHAIN_DISABLED")),
                )
            })?;
            (
                "0x0000000000000000000000000000000000000000".to_string(), // TODO: Real address
                config.usdc_contract.clone(),
            )
        }
        Chain::Near => {
            let config = state.config.near.as_ref().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse::new("NEAR not configured", "CHAIN_DISABLED")),
                )
            })?;
            (
                "deposit.signal-bot.near".to_string(), // TODO: Real address
                config.usdc_contract.clone(),
            )
        }
        Chain::Solana => {
            let config = state.config.solana.as_ref().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse::new("Solana not configured", "CHAIN_DISABLED")),
                )
            })?;
            (
                "11111111111111111111111111111111".to_string(), // TODO: Real address
                config.usdc_mint.clone(),
            )
        }
    };

    Ok(Json(DepositAddressResponse {
        chain,
        address,
        token: "USDC".to_string(),
        token_contract,
        memo: None, // TODO: For NEAR, include user's phone as memo
    }))
}

/// Get pricing information.
async fn get_pricing(State(state): State<Arc<AppState>>) -> Json<PricingResponse> {
    let config = &state.config.pricing;

    let chains = state
        .config
        .enabled_chains()
        .into_iter()
        .map(|chain| ChainInfo {
            chain,
            enabled: true,
            token: "USDC".to_string(),
            deposit_address: "TODO".to_string(), // TODO: Get from facilitators
        })
        .collect();

    Json(PricingResponse {
        prompt_cost_per_million_tokens: PricingCalculator::format_usdc(
            config.prompt_credits_per_million,
        ),
        completion_cost_per_million_tokens: PricingCalculator::format_usdc(
            config.completion_credits_per_million,
        ),
        minimum_per_message: PricingCalculator::format_usdc(config.minimum_credits_per_message),
        supported_chains: chains,
    })
}

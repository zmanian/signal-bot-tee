//! HTTP request handlers.

use super::types::*;
use super::AppState;
use crate::error::ProxyError;
use crate::registry::{normalize_phone_number, PhoneNumberRecord, RegistrationStatus};
use axum::{
    extract::{Path, State},
    Json,
};
use tracing::{info, warn};

/// Health check endpoint.
pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let registry = state.registry.read().await;
    let signal_healthy = state.signal_client.health_check().await;

    Json(HealthResponse {
        status: "ok".to_string(),
        registry_count: registry.count(),
        signal_api_healthy: signal_healthy,
    })
}

/// Initiate registration for a phone number.
pub async fn register_number(
    State(state): State<AppState>,
    Path(number): Path<String>,
    Json(request): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, ProxyError> {
    // Normalize phone number
    let number = normalize_phone_number(&number).map_err(ProxyError::InvalidPhoneNumber)?;
    info!(phone_number = %number, "Registration request received");

    // Check if already registered
    let registry = state.registry.read().await;
    if let Some(record) = registry.get(&number) {
        match record.status {
            RegistrationStatus::Verified => {
                warn!(phone_number = %number, "Attempted re-registration of verified number");
                return Err(ProxyError::AlreadyRegistered(number));
            }
            RegistrationStatus::Pending => {
                // Allow retry if ownership matches
                if !record.verify_ownership(request.ownership_secret.as_deref()) {
                    return Err(ProxyError::OwnershipProofMismatch);
                }
                // Fall through to retry registration
            }
            RegistrationStatus::Failed => {
                // Allow re-registration for failed attempts
            }
        }
    }
    drop(registry);

    // Proxy to Signal CLI REST API
    // If Signal says "already registered", try to unregister first and retry
    let register_result = state
        .signal_client
        .register(&number, request.captcha.as_deref(), request.use_voice)
        .await;

    if let Err(ProxyError::SignalApi(ref msg)) = register_result {
        if msg.contains("already registered") {
            info!(phone_number = %number, "Account already registered, attempting to unregister and retry");
            // Try to unregister the stale registration
            if state.signal_client.unregister(&number).await.is_ok() {
                // Retry registration
                state
                    .signal_client
                    .register(&number, request.captcha.as_deref(), request.use_voice)
                    .await?;
            } else {
                // Unregister failed, return original error
                return Err(register_result.unwrap_err());
            }
        } else {
            return Err(register_result.unwrap_err());
        }
    } else {
        register_result?;
    }

    // Record the registration attempt
    let record = PhoneNumberRecord::new_pending(number.clone(), request.ownership_secret.as_deref());

    let mut registry = state.registry.write().await;
    registry.insert(number.clone(), record);

    // Persist to encrypted storage
    state.store.save(&registry).await?;

    info!(phone_number = %number, "Registration initiated, awaiting verification");

    Ok(Json(RegisterResponse {
        phone_number: number,
        status: "pending".to_string(),
        message: "Verification code sent. Use /v1/register/{number}/verify/{code} to complete."
            .to_string(),
    }))
}

/// Verify registration with code.
pub async fn verify_registration(
    State(state): State<AppState>,
    Path((number, code)): Path<(String, String)>,
    Json(request): Json<VerifyRequest>,
) -> Result<Json<VerifyResponse>, ProxyError> {
    // Normalize phone number
    let number = normalize_phone_number(&number).map_err(ProxyError::InvalidPhoneNumber)?;
    info!(phone_number = %number, "Verification request received");

    // Check registration exists and is pending
    let registry = state.registry.read().await;
    let record = registry.get(&number).ok_or(ProxyError::NotFound(number.clone()))?;

    if record.status != RegistrationStatus::Pending {
        return Err(ProxyError::NotFound(number));
    }

    // Verify ownership
    if !record.verify_ownership(request.ownership_secret.as_deref()) {
        return Err(ProxyError::OwnershipProofMismatch);
    }
    drop(registry);

    // Submit verification code to Signal CLI
    state
        .signal_client
        .verify(&number, &code, request.pin.as_deref())
        .await?;

    // Mark as verified
    let mut registry = state.registry.write().await;
    if let Some(record) = registry.get_mut(&number) {
        record.mark_verified();
    }

    // Persist
    state.store.save(&registry).await?;

    info!(phone_number = %number, "Registration verified successfully");

    Ok(Json(VerifyResponse {
        phone_number: number,
        status: "verified".to_string(),
        message: "Phone number registered successfully.".to_string(),
    }))
}

/// Get registration status for a phone number.
pub async fn get_status(
    State(state): State<AppState>,
    Path(number): Path<String>,
) -> Result<Json<StatusResponse>, ProxyError> {
    let number = normalize_phone_number(&number).map_err(ProxyError::InvalidPhoneNumber)?;

    let registry = state.registry.read().await;
    let record = registry.get(&number).ok_or(ProxyError::NotFound(number.clone()))?;

    Ok(Json(StatusResponse {
        phone_number: record.phone_number.clone(),
        status: record.status.clone(),
        registered_at: Some(record.registered_at.to_rfc3339()),
    }))
}

/// List all registered accounts.
pub async fn list_accounts(State(state): State<AppState>) -> Json<AccountsResponse> {
    let registry = state.registry.read().await;
    let accounts: Vec<AccountInfo> = registry
        .list_all()
        .into_iter()
        .map(|r| AccountInfo {
            phone_number: r.phone_number.clone(),
            status: r.status.clone(),
            registered_at: r.registered_at.to_rfc3339(),
        })
        .collect();

    let total = accounts.len();
    Json(AccountsResponse { accounts, total })
}

/// Unregister a phone number.
pub async fn unregister(
    State(state): State<AppState>,
    Path(number): Path<String>,
    Json(request): Json<UnregisterRequest>,
) -> Result<Json<VerifyResponse>, ProxyError> {
    let number = normalize_phone_number(&number).map_err(ProxyError::InvalidPhoneNumber)?;
    info!(phone_number = %number, "Unregister request received");

    // Check registration exists
    let registry = state.registry.read().await;
    let record = registry.get(&number).ok_or(ProxyError::NotFound(number.clone()))?;

    // Verify ownership
    if !record.verify_ownership(request.ownership_secret.as_deref()) {
        return Err(ProxyError::OwnershipProofMismatch);
    }
    drop(registry);

    // Unregister from Signal CLI
    state.signal_client.unregister(&number).await?;

    // Remove from registry
    let mut registry = state.registry.write().await;
    registry.remove(&number);

    // Persist
    state.store.save(&registry).await?;

    info!(phone_number = %number, "Phone number unregistered");

    Ok(Json(VerifyResponse {
        phone_number: number,
        status: "unregistered".to_string(),
        message: "Phone number unregistered successfully.".to_string(),
    }))
}

/// Debug endpoint: List accounts registered in Signal CLI (not our registry).
pub async fn debug_signal_accounts(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ProxyError> {
    let accounts = state.signal_client.list_accounts().await?;
    Ok(Json(serde_json::json!({
        "signal_cli_accounts": accounts,
        "note": "These are accounts Signal CLI knows about, not our proxy registry"
    })))
}

/// Debug endpoint: Force unregister a number from Signal CLI (bypasses registry check).
pub async fn debug_force_unregister(
    State(state): State<AppState>,
    Path(number): Path<String>,
) -> Result<Json<serde_json::Value>, ProxyError> {
    let number = normalize_phone_number(&number).map_err(ProxyError::InvalidPhoneNumber)?;
    warn!(phone_number = %number, "Force unregister requested (debug endpoint)");

    state.signal_client.unregister(&number).await?;

    // Also remove from our registry if present
    let mut registry = state.registry.write().await;
    registry.remove(&number);
    state.store.save(&registry).await?;

    Ok(Json(serde_json::json!({
        "status": "unregistered",
        "phone_number": number,
        "message": "Force unregistered from Signal CLI"
    })))
}

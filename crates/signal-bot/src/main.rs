//! Signal AI Proxy Bot - Main entry point.

mod commands;
mod config;
mod error;

use crate::commands::*;
use crate::config::Config;
use crate::error::AppResult;
use anyhow::Context;
use conversation_store::ConversationStore;
use dstack_client::DstackClient;
use near_ai_client::NearAiClient;
use signal_client::{MessageReceiver, SignalClient};
use std::sync::Arc;
use tokio::signal;
use tokio_stream::StreamExt;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> AppResult<()> {
    // Load configuration
    let config = Config::load().context("Failed to load configuration")?;

    // Initialize logging
    init_logging(&config.bot.log_level);

    info!("Starting Signal AI Proxy Bot...");

    // Initialize clients
    let near_ai = Arc::new(
        NearAiClient::new(
            &config.near_ai.api_key,
            &config.near_ai.base_url,
            &config.near_ai.model,
            config.near_ai.timeout,
        )
        .context("Failed to create NEAR AI client")?,
    );

    let conversations = Arc::new(ConversationStore::new(
        config.conversation.max_messages,
        config.conversation.ttl,
    ));

    let dstack = Arc::new(DstackClient::new(&config.dstack.socket_path));

    let signal = SignalClient::new(&config.signal.service_url)
        .context("Failed to create Signal client")?;

    // Health checks
    if near_ai.health_check().await {
        info!("NEAR AI healthy - Model: {}", config.near_ai.model);
    } else {
        warn!("NEAR AI health check failed - will retry on requests");
    }

    info!(
        "In-memory conversation store ready (max_messages={}, ttl={:?})",
        config.conversation.max_messages, config.conversation.ttl
    );

    if dstack.is_in_tee().await {
        if let Ok(info) = dstack.get_app_info().await {
            info!(
                "Running in TEE - App ID: {}",
                info.app_id.as_deref().unwrap_or("unknown")
            );
        }
    } else {
        warn!("Not running in TEE environment - attestation unavailable");
    }

    if !signal.health_check().await {
        error!("Signal API not reachable at {}", config.signal.service_url);
        return Err(anyhow::anyhow!("Signal API not reachable").into());
    }
    info!("Signal API healthy");

    // Create command handlers
    let handlers: Vec<Box<dyn CommandHandler>> = vec![
        Box::new(ChatHandler::new(
            near_ai.clone(),
            conversations.clone(),
            config.bot.system_prompt.clone(),
        )),
        Box::new(VerifyHandler::new(dstack.clone())),
        Box::new(ClearHandler::new(conversations.clone())),
        Box::new(HelpHandler::new()),
        Box::new(ModelsHandler::new(near_ai.clone())),
    ];

    info!("Registered {} command handlers", handlers.len());
    info!("NEAR AI endpoint: {}", config.near_ai.base_url);
    info!("Listening for messages...");

    // Start message receiver
    let receiver = MessageReceiver::new(signal.clone(), config.signal.poll_interval);
    let mut stream = Box::pin(receiver.stream());

    // Main message loop
    loop {
        tokio::select! {
            Some(message) = stream.next() => {
                // Find matching handler
                let handler = handlers
                    .iter()
                    .find(|h| h.matches(&message));

                if let Some(handler) = handler {
                    match handler.execute(&message).await {
                        Ok(response) => {
                            if let Err(e) = signal.reply(&message, &response).await {
                                error!("Failed to send reply: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Handler error: {}", e);
                            let _ = signal
                                .reply(&message, "Sorry, something went wrong.")
                                .await;
                        }
                    }
                }
            }
            _ = signal::ctrl_c() => {
                info!("Shutdown signal received");
                break;
            }
        }
    }

    info!("Shutting down...");
    Ok(())
}

fn init_logging(level: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

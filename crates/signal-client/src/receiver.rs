//! Message receiver with polling.

use crate::client::SignalClient;
use crate::types::*;
use std::time::Duration;
use tokio::time::sleep;
use tokio_stream::Stream;
use tracing::{debug, error};

/// Message receiver that polls for new messages.
pub struct MessageReceiver {
    client: SignalClient,
    poll_interval: Duration,
}

impl MessageReceiver {
    /// Create a new message receiver.
    pub fn new(client: SignalClient, poll_interval: Duration) -> Self {
        Self {
            client,
            poll_interval,
        }
    }

    /// Start receiving messages as an async stream.
    pub fn stream(self) -> impl Stream<Item = BotMessage> {
        async_stream::stream! {
            loop {
                match self.client.receive().await {
                    Ok(messages) => {
                        for msg in messages {
                            if let Some(bot_msg) = BotMessage::from_incoming(&msg) {
                                debug!("Received: {} from {}",
                                    &bot_msg.text[..bot_msg.text.len().min(50)],
                                    bot_msg.source
                                );
                                yield bot_msg;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Receive error: {}", e);
                        // Back off on error
                        sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                }

                sleep(self.poll_interval).await;
            }
        }
    }
}

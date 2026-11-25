use std::time::Instant;

use futures::StreamExt;
use tokio::sync::mpsc;

use crate::config::connections::ConnectionProfile;
use crate::redis::client::RedisClientHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PubSubKind {
    Channel(String),
    Pattern(String),
}

impl PubSubKind {
    pub fn name(&self) -> &str {
        match self {
            PubSubKind::Channel(n) => n,
            PubSubKind::Pattern(n) => n,
        }
    }

    pub fn is_pattern(&self) -> bool {
        matches!(self, PubSubKind::Pattern(_))
    }
}

#[derive(Debug, Clone)]
pub struct PubSubMessage {
    pub channel: String,
    pub pattern: Option<String>,
    pub payload: String,
    pub timestamp: Instant,
}

impl PubSubMessage {
    pub fn relative_time(&self) -> String {
        let secs = self.timestamp.elapsed().as_secs();
        if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            format!("{}m", secs / 60)
        } else if secs < 86400 {
            format!("{}h", secs / 3600)
        } else {
            format!("{}d", secs / 86400)
        }
    }
}

pub struct PubSubClient;

impl PubSubClient {
    /// Spawn a new pub/sub subscription task and return a receiver for messages.
    ///
    /// This replaces any existing subscription receiver in the caller; only one
    /// active subscription stream is supported at a time.
    /// Open the subscription and hand back its message stream.
    ///
    /// The connect + subscribe handshake runs here, before the streaming
    /// task is spawned, so a failure is returned to the caller and shown.
    /// Previously this returned `Ok(rx)` immediately and any failure inside
    /// the spawned task died silently: the app reported "Subscribed" while
    /// no messages would ever arrive.
    pub async fn spawn_subscription(
        profile: &ConnectionProfile,
        kind: &PubSubKind,
    ) -> color_eyre::Result<mpsc::UnboundedReceiver<PubSubMessage>> {
        let url = RedisClientHandle::connection_url(profile)
            .map_err(|e| color_eyre::eyre::eyre!(e.to_string()))?;
        let client = redis::Client::open(url)
            .map_err(|e| color_eyre::eyre::eyre!("invalid pub/sub URL: {e}"))?;
        let mut pubsub = client
            .get_async_pubsub()
            .await
            .map_err(|e| color_eyre::eyre::eyre!("pub/sub connect failed: {e}"))?;

        match kind {
            PubSubKind::Channel(ch) => pubsub.subscribe(ch.clone()).await,
            PubSubKind::Pattern(pat) => pubsub.psubscribe(pat.clone()).await,
        }
        .map_err(|e| color_eyre::eyre::eyre!("subscribe failed: {e}"))?;

        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut stream = pubsub.on_message();
            while let Some(msg) = stream.next().await {
                let payload = msg
                    .get_payload::<String>()
                    .unwrap_or_else(|_| "<non-utf8 payload>".to_string());

                let _ = tx.send(PubSubMessage {
                    channel: msg.get_channel_name().to_string(),
                    pattern: msg.get_pattern::<String>().ok(),
                    payload,
                    timestamp: Instant::now(),
                });
            }
        });

        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pubsub_kind_name_returns_inner_string() {
        assert_eq!(PubSubKind::Channel("foo".to_string()).name(), "foo");
        assert_eq!(PubSubKind::Pattern("bar*".to_string()).name(), "bar*");
    }

    #[test]
    fn pubsub_kind_is_pattern_detects_variant() {
        assert!(!PubSubKind::Channel("foo".to_string()).is_pattern());
        assert!(PubSubKind::Pattern("bar*".to_string()).is_pattern());
    }

    #[test]
    fn pubsub_message_relative_time_formats_correctly() {
        let msg = PubSubMessage {
            channel: "ch".to_string(),
            pattern: None,
            payload: "hello".to_string(),
            timestamp: Instant::now(),
        };
        let rt = msg.relative_time();
        assert!(rt.ends_with('s'));
    }
}

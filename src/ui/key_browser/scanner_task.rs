use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::redis::client::{RedisClientHandle, RedisType};
use crate::redis::scanner::Scanner;
use crate::ui::key_browser::tree::KeyEntry;

/// Start a background SCAN task. Returns a cancellation token and a receiver.
/// The sender is dropped on completion or cancellation, so the receiver sees None.
pub fn start_scan(
    client: RedisClientHandle,
    config: &Config,
) -> (mpsc::Receiver<Vec<KeyEntry>>, CancellationToken) {
    let count = config.scan_count();
    let (tx, rx) = mpsc::channel(64);
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    // The scanner takes ownership of the handle; keep a clone for the
    // TYPE/TTL hydration pass below.
    let hydrate_client = client.clone();

    tokio::spawn(async move {
        let mut scanner = Scanner::new(client, count);
        loop {
            if cancel_clone.is_cancelled() {
                break;
            }
            match scanner.next_batch().await {
                Ok(keys) => {
                    // Every entry previously arrived with `redis_type: None`
                    // and `ttl: None`, and nothing ever filled them in: no
                    // type badges, an empty TTL sort, and inspection failing
                    // with "unknown redis type" for every key. Hydrate the
                    // batch in one pipelined round-trip; on failure, degrade
                    // to untyped entries rather than dropping keys.
                    let hydrated: Vec<_> = hydrate_client
                        .key_types_and_ttls(&keys)
                        .await
                        .unwrap_or_default();
                    let batch: Vec<KeyEntry> = keys
                        .into_iter()
                        .enumerate()
                        .map(|(i, k)| {
                            let (redis_type, ttl) = hydrated
                                .get(i)
                                .cloned()
                                .unwrap_or((RedisType::Unknown, None));
                            KeyEntry {
                                full_name: k,
                                // Unknown means "never resolved" — the app
                                // re-fetches TYPE at inspection time, so
                                // even a failed hydration cannot break Enter.
                                redis_type: Some(redis_type),
                                ttl,
                            }
                        })
                        .collect();
                    if batch.is_empty() && scanner.finished {
                        break;
                    }
                    // Non-blocking send; if channel is full the batch is dropped (backpressure)
                    if mpsc::Sender::try_send(&tx, batch).is_err() {
                        break;
                    }
                    if scanner.finished {
                        break;
                    }
                }
                Err(_) => {
                    break;
                }
            }
        }
    });

    (rx, cancel)
}

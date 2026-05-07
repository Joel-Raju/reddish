use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::redis::client::RedisClientHandle;
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

    tokio::spawn(async move {
        let mut scanner = Scanner::new(client, count);
        loop {
            if cancel_clone.is_cancelled() {
                break;
            }
            match scanner.next_batch().await {
                Ok(keys) => {
                    let batch: Vec<KeyEntry> = keys
                        .into_iter()
                        .map(|k| KeyEntry {
                            full_name: k,
                            redis_type: None,
                            ttl: None,
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

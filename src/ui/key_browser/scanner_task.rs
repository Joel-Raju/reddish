use tokio::sync::mpsc;

use crate::config::Config;
use crate::redis::client::RedisClientHandle;
use crate::redis::scanner::Scanner;
use crate::ui::key_browser::tree::KeyEntry;

pub fn start_scan(client: RedisClientHandle, config: &Config) -> mpsc::Receiver<Vec<KeyEntry>> {
    let count = config.scan_count();
    let (tx, rx) = mpsc::channel(10);

    tokio::spawn(async move {
        let mut scanner = Scanner::new(client, count);
        while let Ok(keys) = scanner.next_batch().await {
            let batch: Vec<KeyEntry> = keys.into_iter()
                .map(|k| KeyEntry { full_name: k, redis_type: None, ttl: None })
                .collect();
            if tx.send(batch).await.is_err() {
                break;
            }
            if scanner.finished {
                break;
            }
        }
    });

    rx
}

use color_eyre::Result;

use crate::redis::client::RedisClientHandle;

pub struct Scanner {
    client: RedisClientHandle,
    cursor: u64,
    pattern: Option<String>,
    count: u32,
    pub finished: bool,
}

impl Scanner {
    pub fn new(client: RedisClientHandle, count: u32) -> Self {
        Self {
            client,
            cursor: 0,
            pattern: None,
            count,
            finished: false,
        }
    }

    pub async fn next_batch(&mut self) -> Result<Vec<String>> {
        if self.finished {
            return Ok(Vec::new());
        }

        let pattern = self.pattern.as_deref().unwrap_or("*");
        let mut conn = match &self.client.client {
            crate::redis::client::RedisClient::Standalone(c) => c.clone(),
        };

        let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(self.cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(self.count)
            .query_async(&mut conn)
            .await
            .map_err(|e| color_eyre::eyre::eyre!("SCAN failed: {}", e))?;

        self.cursor = new_cursor;
        if self.cursor == 0 {
            self.finished = true;
        }

        Ok(keys)
    }
}

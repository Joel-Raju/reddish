use std::time::Duration;

use color_eyre::Result;
use redis::aio::MultiplexedConnection;

use crate::redis::client::RedisClientHandle;

const SCAN_TIMEOUT: Duration = Duration::from_secs(5);

/// State for iterating a single node's cursor loop.
struct NodeScan {
    conn: MultiplexedConnection,
    cursor: u64,
    done: bool,
}

pub struct Scanner {
    client: RedisClientHandle,
    pattern: Option<String>,
    count: u32,
    pub finished: bool,
    /// Standalone mode: single cursor tracked here.
    standalone_cursor: u64,
    /// Cluster mode: lazily-populated list of per-node scan states.
    cluster_nodes: Option<Vec<NodeScan>>,
    /// Which cluster node we are currently iterating.
    cluster_node_idx: usize,
}

impl Scanner {
    pub fn new(client: RedisClientHandle, count: u32) -> Self {
        Self {
            client,
            pattern: None,
            count,
            finished: false,
            standalone_cursor: 0,
            cluster_nodes: None,
            cluster_node_idx: 0,
        }
    }

    pub async fn next_batch(&mut self) -> Result<Vec<String>> {
        if self.finished {
            return Ok(Vec::new());
        }

        match &self.client.client {
            crate::redis::client::RedisClient::Standalone(_) => {
                self.next_batch_standalone().await
            }
            crate::redis::client::RedisClient::Cluster(_) => {
                self.next_batch_cluster().await
            }
        }
    }

    async fn next_batch_standalone(&mut self) -> Result<Vec<String>> {
        let pattern = self.pattern.as_deref().unwrap_or("*");
        let mut cmd = redis::cmd("SCAN");
        cmd.arg(self.standalone_cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(self.count);

        let conn_clone = match &self.client.client {
            crate::redis::client::RedisClient::Standalone(c) => c.clone(),
            _ => unreachable!(),
        };
        let mut conn = conn_clone;

        let (new_cursor, keys): (u64, Vec<String>) =
            tokio::time::timeout(SCAN_TIMEOUT, cmd.query_async::<(u64, Vec<String>)>(&mut conn))
                .await
                .map_err(|_| color_eyre::eyre::eyre!("SCAN timed out after {}s", SCAN_TIMEOUT.as_secs()))?
                .map_err(|e| color_eyre::eyre::eyre!("SCAN failed: {}", e))?;

        self.standalone_cursor = new_cursor;
        if self.standalone_cursor == 0 {
            self.finished = true;
        }
        Ok(keys)
    }

    async fn next_batch_cluster(&mut self) -> Result<Vec<String>> {
        // Lazy-initialise per-node connections from CLUSTER NODES.
        if self.cluster_nodes.is_none() {
            let addrs = self.client.cluster_master_addrs().await?;
            if addrs.is_empty() {
                self.finished = true;
                return Ok(Vec::new());
            }
            let mut nodes = Vec::with_capacity(addrs.len());
            for addr in &addrs {
                let conn = self.client.connect_node(addr).await?;
                nodes.push(NodeScan { conn, cursor: 0, done: false });
            }
            self.cluster_nodes = Some(nodes);
        }

        let pattern = self.pattern.as_deref().unwrap_or("*");
        let nodes = self.cluster_nodes.as_mut().unwrap();

        // Advance past already-exhausted nodes.
        while self.cluster_node_idx < nodes.len() && nodes[self.cluster_node_idx].done {
            self.cluster_node_idx += 1;
        }

        if self.cluster_node_idx >= nodes.len() {
            self.finished = true;
            return Ok(Vec::new());
        }

        let node = &mut nodes[self.cluster_node_idx];
        let mut cmd = redis::cmd("SCAN");
        cmd.arg(node.cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(self.count);

        let (new_cursor, keys): (u64, Vec<String>) =
            tokio::time::timeout(SCAN_TIMEOUT, cmd.query_async::<(u64, Vec<String>)>(&mut node.conn))
                .await
                .map_err(|_| color_eyre::eyre::eyre!("SCAN timed out after {}s", SCAN_TIMEOUT.as_secs()))?
                .map_err(|e| color_eyre::eyre::eyre!("SCAN failed: {}", e))?;

        node.cursor = new_cursor;
        if node.cursor == 0 {
            node.done = true;
            self.cluster_node_idx += 1;
            if self.cluster_node_idx >= nodes.len() {
                self.finished = true;
            }
        }
        Ok(keys)
    }
}

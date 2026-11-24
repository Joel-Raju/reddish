//! Shared helpers for the Redis-backed integration tests.
//!
//! The suite previously hard-required a server on 127.0.0.1:16379 with no
//! gating whatsoever: 38 tests were red on any clean checkout, which
//! trains you to ignore red. `connect_or_skip` keeps CI (which provisions
//! the server) running every test, while a local run without one prints a
//! visible SKIP instead of failing.

use std::time::Duration;

use reddish_tui::config::connections::ConnectionProfile;
use reddish_tui::redis::client::RedisClientHandle;

/// Connect to the integration Redis, or skip the test loudly when absent.
///
/// Returns `None` after printing why, so the caller can early-return. A
/// skip is visible in the run output; a failure on a serverless machine
/// is indistinguishable from a regression and buries the real ones.
pub async fn connect_or_skip(profile: &ConnectionProfile) -> Option<RedisClientHandle> {
    let connected =
        tokio::time::timeout(Duration::from_secs(2), RedisClientHandle::connect(profile)).await;
    let addr = format!("{}:{}", profile.host, profile.port);
    match connected {
        Ok(Ok(client)) => Some(client),
        Ok(Err(err)) => {
            eprintln!("SKIP: no Redis at {addr} ({err}); start one to run this test");
            None
        }
        Err(_) => {
            eprintln!("SKIP: Redis at {addr} did not answer within 2s");
            None
        }
    }
}

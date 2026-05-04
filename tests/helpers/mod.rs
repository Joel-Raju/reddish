use std::process::{Child, Command};
use std::time::Duration;
use tokio::time::sleep;

/// Spawn a test Redis server on port 16379.
/// Returns the Child handle (drop it to kill the server).
pub async fn redis_test_server() -> Child {
    let mut child = Command::new("redis-server")
        .args(["--port", "16379", "--daemonize", "no", "--loglevel", "warning"])
        .spawn()
        .expect("Failed to start redis-server. Is it installed?");

    // Wait for Redis to accept connections
    for _ in 0..50 {
        sleep(Duration::from_millis(100)).await;
        if let Ok(mut client) = redis::Client::open("redis://127.0.0.1:16379") {
            if client.get_connection().is_ok() {
                break;
            }
        }
    }
    child
}

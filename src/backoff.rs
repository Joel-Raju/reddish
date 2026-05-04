use std::time::Duration;

/// Returns a back-off sequence with ±10% jitter.
/// Sequence: [100, 200, 400, 800, 1600, 5000, 30000] ms
pub fn backoff_sequence() -> Vec<Duration> {
    let base = vec![100, 200, 400, 800, 1600, 5000, 30000];
    base.into_iter()
        .map(|ms| {
            let jitter = (ms as f64 * 0.1) as u64;
            let adjusted = ms + jitter;
            Duration::from_millis(adjusted)
        })
        .collect()
}

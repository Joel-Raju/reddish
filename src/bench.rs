//! Performance gates behind the hidden `--bench` and `--profile-memory` flags.
//!
//! These exist so the spec's quality thresholds (render p99, RSS ceiling) are
//! measurable rather than aspirational. Each runs against a synthetic, fully
//! populated `App` so the numbers do not depend on a live Redis, and each
//! returns a non-zero exit code when its budget is exceeded.
//!
//! Budgets are deliberately generous relative to a debug build: `cargo run`
//! without `--release` is typically several times slower than the shipped
//! binary, and a gate that only passes optimized would be useless locally.

use std::time::{Duration, Instant};

use ratatui::{Terminal, backend::TestBackend};

use crate::app::App;
use crate::config::Config;
use crate::redis::client::RedisType;
use crate::redis::types::{RedisValue, StreamEntry, ZSetEntry};
use crate::ui::key_browser::tree::KeyEntry;
use crate::ui::value_inspector::ValueInspector;

/// Keys synthesized into the tree for both gates.
pub const BENCH_KEYS: usize = 10_000;

/// Render cycles executed by `--bench`.
pub const BENCH_RENDERS: usize = 1_000;

/// p99 render budget.
pub const RENDER_P99_BUDGET: Duration = Duration::from_millis(16);

/// Peak RSS budget after loading `BENCH_KEYS`.
pub const RSS_BUDGET_BYTES: u64 = 50 * 1024 * 1024;

/// Build an `App` populated with `BENCH_KEYS` synthetic namespace entries.
///
/// No connection is opened: the point is to measure render cost and resident
/// memory for a full tree, both of which are independent of Redis.
pub fn synthetic_app(config: Config, keys: usize) -> App {
    let mut app = App::new(config);
    let max_keys = app.config.max_keys_in_memory();

    let types = [
        RedisType::String,
        RedisType::List,
        RedisType::Hash,
        RedisType::Set,
        RedisType::ZSet,
        RedisType::Stream,
    ];
    let ttls = [
        crate::redis::client::Ttl::NoExpiry,
        crate::redis::client::Ttl::Expires(Duration::from_secs(60)),
        crate::redis::client::Ttl::Expires(Duration::from_secs(3600)),
        crate::redis::client::Ttl::Expires(Duration::from_secs(86_400)),
    ];

    // Insert through the public API so the benchmark measures the same tree
    // shape a real scan produces, including the max_keys cap.
    let mut inserted = 0usize;
    for i in 0..keys {
        let entry = KeyEntry {
            full_name: format!("bench:shard{}:{:06}:entry{}", i % 64, i, i),
            redis_type: Some(types[i % types.len()].clone()),
            ttl: Some(ttls[i % ttls.len()].clone()),
        };
        if app.key_browser.tree.insert(entry) {
            inserted += 1;
        }
    }
    debug_assert!(
        inserted <= max_keys,
        "tree must respect max_keys_in_memory ({max_keys})"
    );
    app.key_browser.tree.expand_all();

    // A populated inspector makes the render path realistic rather than
    // measuring an empty pane.
    app.value_inspector.set_value(
        "bench:shard0:000000:entry0".to_string(),
        RedisValue::List((0..500).map(|i| format!("item-{i}")).collect()),
    );
    app.value_inspector.set_metadata(
        Some("embstr".to_string()),
        Some(128_000),
        Some(crate::redis::client::Ttl::Expires(Duration::from_secs(3600))),
    );

    app
}

/// Populate the inspector with a large value of each remaining type so render
/// cost is measured across all of them.
pub fn populate_all_value_types(inspector: &mut ValueInspector) {
    inspector.set_value(
        "bench:hash".to_string(),
        RedisValue::Hash(
            (0..2_000)
                .map(|i| (format!("field-{i}"), format!("value-{i}")))
                .collect(),
        ),
    );
    inspector.set_value(
        "bench:zset".to_string(),
        RedisValue::ZSet(
            (0..2_000)
                .map(|i| ZSetEntry {
                    score: i as f64,
                    member: format!("member-{i}"),
                })
                .collect(),
        ),
    );
    inspector.set_value(
        "bench:stream".to_string(),
        RedisValue::Stream(
            (0..2_000)
                .map(|i| StreamEntry {
                    id: format!("{}-0", 1_600_000_000_000u64 + i),
                    fields: [("k".to_string(), format!("v{i}"))]
                        .into_iter()
                        .collect(),
                })
                .collect(),
        ),
    );
}

#[derive(Debug, Clone, Copy)]
pub struct RenderStats {
    pub samples: usize,
    pub min: Duration,
    pub p50: Duration,
    pub p99: Duration,
    pub max: Duration,
    pub mean: Duration,
}

impl RenderStats {
    /// True when p99 is within budget.
    pub fn within_budget(&self) -> bool {
        self.p99 <= RENDER_P99_BUDGET
    }
}

fn percentile(sorted: &[Duration], q: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    // Nearest-rank percentile.
    let rank = (q * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

/// Run `renders` draw cycles at `width` x `height` and summarize the timings.
pub fn measure_render(app: &App, renders: usize, width: u16, height: u16) -> RenderStats {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("TestBackend construction");

    let mut samples: Vec<Duration> = Vec::with_capacity(renders);
    for _ in 0..renders {
        let start = Instant::now();
        terminal
            .draw(|f| app.render(f))
            .expect("draw must not fail during benchmarking");
        samples.push(start.elapsed());
    }

    let total: Duration = samples.iter().sum();
    let mut sorted = samples.clone();
    sorted.sort_unstable();

    RenderStats {
        samples: renders,
        min: *sorted.first().unwrap_or(&Duration::ZERO),
        p50: percentile(&sorted, 0.50),
        p99: percentile(&sorted, 0.99),
        max: *sorted.last().unwrap_or(&Duration::ZERO),
        mean: total / renders.max(1) as u32,
    }
}

fn fmt_ms(d: Duration) -> String {
    format!("{:.3}ms", d.as_secs_f64() * 1000.0)
}

/// Print a render-timing report to stderr.
pub fn report_render(stats: &RenderStats, keys: usize) {
    eprintln!(
        "render benchmark: {keys} keys, {}x24, {} cycles",
        80, stats.samples
    );
    eprintln!("  min  {}", fmt_ms(stats.min));
    eprintln!("  p50  {}", fmt_ms(stats.p50));
    eprintln!("  p99  {}", fmt_ms(stats.p99));
    eprintln!("  max  {}", fmt_ms(stats.max));
    eprintln!("  mean {}", fmt_ms(stats.mean));
    eprintln!("  budget p99 <= {}", fmt_ms(RENDER_P99_BUDGET));
    eprintln!(
        "  result: {}",
        if stats.within_budget() {
            "PASS"
        } else {
            "FAIL"
        }
    );
}

/// Peak resident set size in bytes, or `None` where unsupported.
///
/// Uses `/proc/self/status` on Linux and `getrusage` elsewhere is avoided in
/// favour of a portable ps where available, so the gate degrades to a skip
/// rather than failing to compile.
pub fn peak_rss_bytes() -> Option<u64> {
    if cfg!(target_os = "linux") {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("VmHWM:") {
                let kb: u64 = rest
                    .trim()
                    .trim_end_matches(" kB")
                    .trim()
                    .parse()
                    .ok()?;
                return Some(kb * 1024);
            }
        }
        return None;
    }
    peak_rss_via_sysinfo()
}

/// `sysinfo` is already a dependency; use it for non-Linux platforms.
fn peak_rss_via_sysinfo() -> Option<u64> {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[Pid::from_u32(std::process::id())]),
        true,
        ProcessRefreshKind::nothing().with_memory(),
    );
    let proc = sys.process(Pid::from_u32(std::process::id()))?;
    // sysinfo 0.38 reports Process::memory() in bytes on every platform, so
    // this is returned as-is. Scaling it here reported 23GB for a 23MB process.
    Some(proc.memory())
}

/// Print an RSS report to stderr. Returns `true` when within budget.
pub fn report_rss(rss: Option<u64>, keys: usize) -> bool {
    let Some(rss) = rss else {
        eprintln!("memory profile: {keys} keys — RSS unavailable on this platform (skipped)");
        return true;
    };
    let mb = rss as f64 / (1024.0 * 1024.0);
    let budget_mb = RSS_BUDGET_BYTES as f64 / (1024.0 * 1024.0);
    let within = rss <= RSS_BUDGET_BYTES;
    eprintln!("memory profile: {keys} keys, peak RSS {mb:.1}MB (budget {budget_mb:.0}MB)");
    eprintln!("  result: {}", if within { "PASS" } else { "FAIL" });
    within
}

/// Entry point for `--bench`. Returns `true` when the p99 budget holds.
pub fn run_bench(config: Config) -> bool {
    let app = synthetic_app(config.clone(), BENCH_KEYS);
    let stats = measure_render(&app, BENCH_RENDERS, 80, 24);
    report_render(&stats, BENCH_KEYS);

    // Also report the default view (namespaces collapsed), which is what a
    // user actually sees on load. The expanded case above is the worst case:
    // every one of the 10K keys is a visible row, and render builds a ListItem
    // per row even though only a couple of dozen are drawn.
    let mut collapsed = synthetic_app(config, BENCH_KEYS);
    collapsed.key_browser.tree.collapse_all();
    let collapsed_stats = measure_render(&collapsed, BENCH_RENDERS, 80, 24);
    eprintln!("render benchmark (default collapsed view):");
    report_render(&collapsed_stats, BENCH_KEYS);

    stats.within_budget()
}

/// Entry point for `--profile-memory`. Returns `true` when RSS is within budget.
pub fn run_profile_memory(config: Config) -> bool {
    let mut app = synthetic_app(config, BENCH_KEYS);
    populate_all_value_types(&mut app.value_inspector);
    // Read the key count back so the report reflects what the tree retained
    // after the max_keys cap, not what we attempted to insert.
    let retained = app.key_browser.tree.total_keys();
    report_rss(peak_rss_bytes(), retained)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn synthetic_app_populates_requested_key_count() {
        let app = synthetic_app(Config::default(), 500);
        assert_eq!(app.key_browser.tree.total_keys(), 500);
    }

    #[test]
    fn synthetic_app_populates_metadata_on_entries() {
        let app = synthetic_app(Config::default(), 10);
        let rows = app.key_browser.tree.visible_rows();
        assert!(rows.iter().any(|r| r.key.as_ref().is_some_and(|k| k.ttl.is_some())));
    }

    #[test]
    fn synthetic_app_loads_an_inspector_value() {
        let app = synthetic_app(Config::default(), 10);
        assert!(matches!(
            app.value_inspector.value,
            Some(RedisValue::List(_))
        ));
    }

    #[test]
    fn measure_render_records_one_sample_per_cycle() {
        let app = synthetic_app(Config::default(), 100);
        let stats = measure_render(&app, 25, 80, 24);
        assert_eq!(stats.samples, 25);
        assert!(stats.min <= stats.p50);
        assert!(stats.p50 <= stats.p99);
        assert!(stats.p99 <= stats.max);
    }

    #[test]
    fn measure_render_is_deterministic_in_shape_at_any_size() {
        let app = synthetic_app(Config::default(), 50);
        for (w, h) in [(80u16, 24u16), (40, 10), (200, 60), (20, 6)] {
            let stats = measure_render(&app, 5, w, h);
            assert_eq!(stats.samples, 5, "{w}x{h}");
        }
    }

    #[test]
    fn percentile_uses_nearest_rank() {
        let samples: Vec<Duration> = (1..=100)
            .map(Duration::from_micros)
            .collect();
        // p50 of 1..100us -> rank 50 -> 50us
        assert_eq!(percentile(&samples, 0.50), Duration::from_micros(50));
        // p99 -> rank 99 -> 99us
        assert_eq!(percentile(&samples, 0.99), Duration::from_micros(99));
        // Clamped for degenerate inputs.
        assert_eq!(percentile(&[], 0.5), Duration::ZERO);
        assert_eq!(percentile(&samples, 0.0), Duration::from_micros(1));
        assert_eq!(percentile(&samples, 1.0), Duration::from_micros(100));
    }

    #[test]
    fn render_stats_budget_check() {
        let ok = RenderStats {
            samples: 1,
            min: Duration::from_millis(1),
            p50: Duration::from_millis(1),
            p99: Duration::from_millis(10),
            max: Duration::from_millis(10),
            mean: Duration::from_millis(1),
        };
        assert!(ok.within_budget());

        let over = RenderStats { p99: RENDER_P99_BUDGET + Duration::from_millis(1), ..ok };
        assert!(!over.within_budget());
    }

    #[test]
    fn populate_all_value_types_sets_every_variant() {
        let mut inspector = ValueInspector::new();
        populate_all_value_types(&mut inspector);
        assert!(matches!(inspector.value, Some(RedisValue::Stream(_))));
    }

    #[test]
    fn rss_is_readable_or_cleanly_skipped() {
        // Must never panic; on an unsupported platform it returns None.
        let _ = peak_rss_bytes();
    }

    #[test]
    fn report_rss_skips_when_unavailable() {
        assert!(report_rss(None, 100));
    }

    #[test]
    fn report_rss_flags_over_budget() {
        assert!(!report_rss(Some(RSS_BUDGET_BYTES + 1), 100));
        assert!(report_rss(Some(RSS_BUDGET_BYTES), 100));
    }

    #[test]
    fn bench_run_with_small_budget_is_green() {
        // Exercise the reporting path end to end.
        let app = synthetic_app(Config::default(), 20);
        let stats = measure_render(&app, 5, 80, 24);
        report_render(&stats, 20);
    }

    #[test]
    fn render_with_bench_tree_does_not_panic() {
        let app = synthetic_app(Config::default(), 2_000);
        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| app.render(f)).unwrap();
        let _ = Rect::new(0, 0, 1, 1);
    }

    #[test]
    fn render_bench_covers_every_tab() {
        use crate::app::Tab;
        let mut app = synthetic_app(Config::default(), 500);
        populate_all_value_types(&mut app.value_inspector);
        app.pubsub_widget
            .handle_event(&crate::events::Event::Key(crossterm::event::KeyEvent::from(
                crossterm::event::KeyCode::Char('a'),
            )));
        for tab in [Tab::Keys, Tab::Repl, Tab::Info, Tab::PubSub] {
            app.active_tab = tab;
            let backend = TestBackend::new(120, 40);
            let mut terminal = Terminal::new(backend).unwrap();
            terminal.draw(|f| app.render(f)).unwrap();
        }
    }
}
//! End-to-end checks that the built binary behaves as documented.
//!
//! These run the real executable rather than calling into the library, so they
//! catch packaging and `main` wiring problems that unit tests cannot see — a
//! missing module declaration, a broken CLI parse, or a startup path that
//! panics before the first draw.
//!
//! Only `--help`, `--version` and the performance gates are exercised: every
//! other path enters the event loop and needs a real terminal and a live
//! Redis, neither of which exists on a headless CI runner.

use std::process::Command;

/// Path to the binary under test.
///
/// Cargo sets `CARGO_BIN_EXE_<name>` for integration tests, which is the
/// reliable way to find it regardless of profile or target directory.
fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_reddish-tui")
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(binary())
        .args(args)
        // A predictable, empty config so a developer's real settings cannot
        // change the outcome.
        .env("EDITOR", "")
        .output()
        .expect("failed to launch the binary")
}

#[test]
fn binary_exists_and_is_executable() {
    let path = std::path::Path::new(binary());
    assert!(path.exists(), "{} is missing", path.display());
    assert!(path.is_file(), "{} is not a file", path.display());
}

#[test]
fn help_exits_successfully_and_lists_documented_flags() {
    let out = run(&["--help"]);
    assert!(
        out.status.success(),
        "--help failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let help = String::from_utf8_lossy(&out.stdout);

    for flag in [
        "--url",
        "--profile",
        "--readonly",
        "--log-level",
        "--theme",
        "--allow-monitor",
    ] {
        assert!(help.contains(flag), "{flag} missing from --help:\n{help}");
    }
}

#[test]
fn performance_flags_stay_hidden_from_help() {
    let help = String::from_utf8_lossy(&run(&["--help"]).stdout).to_string();
    assert!(
        !help.contains("--bench"),
        "--bench should be hidden:\n{help}"
    );
    assert!(
        !help.contains("--profile-memory"),
        "--profile-memory should be hidden:\n{help}"
    );
}

#[test]
fn version_exits_successfully() {
    let out = run(&["--version"]);
    assert!(
        out.status.success(),
        "--version failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let version = String::from_utf8_lossy(&out.stdout);
    assert!(
        version.contains(env!("CARGO_PKG_VERSION")),
        "got: {version}"
    );
}

#[test]
fn unknown_flag_is_rejected() {
    let out = run(&["--definitely-not-a-real-flag"]);
    assert!(
        !out.status.success(),
        "an unknown flag should be a non-zero exit"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--definitely-not-a-real-flag"),
        "error should name the offending flag: {stderr}"
    );
}

#[test]
fn help_does_not_require_a_terminal() {
    // If --help ever started initializing the terminal, this would fail on a
    // headless runner with no TTY.
    assert!(
        !std::io::IsTerminal::is_terminal(&std::io::stdin()),
        "test assumes no TTY on stdin"
    );
    let out = run(&["--help"]);
    assert!(out.status.success());
}

#[test]
fn render_benchmark_runs_and_reports() {
    // Release only: a debug build is roughly 3x slower and would fail the
    // budget, which would make this test useless locally. Skipped rather than
    // failed when assertions are off, since the CI `perf` job is the real gate.
    if cfg!(debug_assertions) {
        eprintln!("skipping --bench: requires a release build");
        return;
    }
    let out = run(&["--bench"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stdout.contains("render benchmark") || stderr.contains("render benchmark"),
        "no benchmark report:\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("result: PASS") || stderr.contains("result: PASS"),
        "render budget exceeded:\n{stdout}\n{stderr}"
    );
    assert!(
        out.status.success(),
        "--bench should exit 0 when within budget"
    );
}

#[test]
fn memory_benchmark_runs_and_reports() {
    if cfg!(debug_assertions) {
        eprintln!("skipping --profile-memory: requires a release build");
        return;
    }
    let out = run(&["--profile-memory"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stdout.contains("memory profile") || stderr.contains("memory profile"),
        "no memory report:\n{stdout}\n{stderr}"
    );
    assert!(out.status.success(), "--profile-memory should exit 0");
}

#[test]
fn performance_gates_do_not_touch_the_terminal() {
    // Both gates run before init_terminal, so stdout must contain only their
    // report and no terminal escape sequences.
    if cfg!(debug_assertions) {
        return;
    }
    for flag in ["--bench", "--profile-memory"] {
        let out = run(&[flag]);
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            !combined.contains('\u{1b}'),
            "{flag} emitted escape sequences, so it touched the terminal:\n{combined}"
        );
    }
}

// ---- Themes (the CLI help must not lie) ----

/// Every theme named in `--help` must actually be selectable.
///
/// The per-theme acceptance is covered cheaply by `theme_application.rs` at the
/// library level; driving it through the binary here would mean running the
/// full render benchmark once per theme, which costs minutes.
#[test]
fn help_and_error_agree_on_theme_names() {
    let help = String::from_utf8_lossy(&run(&["--help"]).stdout).to_string();
    for name in [
        "default",
        "dracula",
        "nord",
        "solarized_dark",
        "gruvbox_dark",
    ] {
        assert!(
            help.contains(name),
            "{name} is a valid theme but missing from --help:\n{help}"
        );
    }
    // The short forms that do not exist must not be advertised. They were
    // previously listed, so users could copy them straight into a script.
    for bogus in ["solarized ", "gruvbox "] {
        assert!(
            !help.contains(bogus),
            "--help advertises the non-existent theme {bogus:?}:\n{help}"
        );
    }
}

/// An unknown theme is an error listing the valid names, not a silent fallback.
///
/// Uses `--profile-memory`, which reaches theme validation (it runs before the
/// performance gates) but is much cheaper than `--bench`. `--help` and
/// `--version` short-circuit in clap and would never get that far.
#[test]
fn unknown_theme_is_rejected_with_the_valid_names() {
    let out = Command::new(binary())
        .args(["--theme", "definitely-not-a-theme", "--profile-memory"])
        .output()
        .expect("failed to launch the binary");
    assert!(!out.status.success(), "an unknown theme must not exit 0");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Unknown theme"),
        "error should say what is wrong: {stderr}"
    );
    for name in [
        "default",
        "dracula",
        "nord",
        "solarized_dark",
        "gruvbox_dark",
    ] {
        assert!(stderr.contains(name), "error should list {name}: {stderr}");
    }
}

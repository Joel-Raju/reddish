//! Command-line interface definition.
//!
//! Lives in the library rather than `main.rs` so the man-page generator
//! (`examples/generate-man.rs`) can build the same [`Command`] the binary
//! parses, guaranteeing the two never drift.

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "redis-tui")]
#[command(about = "A Redis TUI client", long_about = None)]
#[command(version)]
pub struct Cli {
    /// Connect to this Redis URL (e.g. redis://localhost:6379/0)
    #[arg(long, value_name = "URL")]
    pub url: Option<String>,

    /// Connect using a named profile from connections.toml
    #[arg(long, value_name = "NAME")]
    pub profile: Option<String>,

    /// Block all write operations
    #[arg(long)]
    pub readonly: bool,

    /// Log level: error, warn, info, debug, trace
    #[arg(long, value_name = "LEVEL")]
    pub log_level: Option<String>,

    /// Color theme: default, dracula, nord, solarized, gruvbox
    #[arg(long, value_name = "THEME")]
    pub theme: Option<String>,

    /// Allow MONITOR mode (opens a separate connection)
    #[arg(long)]
    pub allow_monitor: bool,

    /// Run 1000 render cycles against a synthetic 10K-key state and report
    /// render-time percentiles. Exits non-zero if p99 exceeds the budget.
    #[arg(long, hide = true)]
    pub bench: bool,

    /// Populate 10K keys and report peak resident set size. Exits non-zero if
    /// RSS exceeds the budget.
    #[arg(long, hide = true)]
    pub profile_memory: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use clap_mangen::Man;

    #[test]
    fn cli_definition_is_valid() {
        // Catches duplicate short flags, conflicting defaults, and malformed
        // help text, all of which panic at runtime.
        Cli::command().debug_assert();
    }

    #[test]
    fn url_flag_parses() {
        let cli = Cli::try_parse_from(["redis-tui", "--url", "redis://localhost:6379"]).unwrap();
        assert_eq!(cli.url.as_deref(), Some("redis://localhost:6379"));
    }

    #[test]
    fn readonly_flag_parses() {
        let cli = Cli::try_parse_from(["redis-tui", "--readonly"]).unwrap();
        assert!(cli.readonly);
    }

    #[test]
    fn allow_monitor_flag_defaults_false() {
        let cli = Cli::try_parse_from(["redis-tui"]).unwrap();
        assert!(!cli.allow_monitor);
        let cli = Cli::try_parse_from(["redis-tui", "--allow-monitor"]).unwrap();
        assert!(cli.allow_monitor);
    }

    #[test]
    fn performance_flags_are_hidden_but_accepted() {
        let cli = Cli::try_parse_from(["redis-tui", "--bench"]).unwrap();
        assert!(cli.bench);
        let cli = Cli::try_parse_from(["redis-tui", "--profile-memory"]).unwrap();
        assert!(cli.profile_memory);

        // Hidden from the short help text users actually read.
        let help = Cli::command().render_help().to_string();
        assert!(!help.contains("--bench"), "bench leaked into help:\n{help}");
        assert!(!help.contains("--profile-memory"), "leaked:\n{help}");
    }

    #[test]
    fn profile_and_theme_flags_parse() {
        let cli = Cli::try_parse_from([
            "redis-tui",
            "--profile",
            "prod",
            "--theme",
            "nord",
            "--log-level",
            "debug",
        ])
        .unwrap();
        assert_eq!(cli.profile.as_deref(), Some("prod"));
        assert_eq!(cli.theme.as_deref(), Some("nord"));
        assert_eq!(cli.log_level.as_deref(), Some("debug"));
    }

    #[test]
    fn unknown_flag_is_rejected() {
        assert!(Cli::try_parse_from(["redis-tui", "--not-a-flag"]).is_err());
    }

    /// The committed man page must match the current flag definitions.
    ///
    /// Regenerate with `cargo run --example generate-man > man/redis-tui.1`
    /// whenever a flag changes.
    #[test]
    fn committed_man_page_is_not_stale() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("man/redis-tui.1");
        let Ok(contents) = std::fs::read_to_string(&path) else {
            panic!("{} is missing; regenerate the man page", path.display());
        };

        let mut buf: Vec<u8> = Vec::new();
        Man::new(Cli::command())
            .render(&mut buf)
            .expect("man renders");
        let expected = String::from_utf8(buf).expect("man page is utf-8");

        // Compare the generated OPTIONS block: it is the part that changes
        // when a flag is added, renamed, or re-documented.
        let options_of = |s: &str| {
            let start = s.find(".SH OPTIONS").expect("OPTIONS section");
            let end = s[start..]
                .find(".SH VERSION")
                .map(|i| start + i)
                .unwrap_or(s.len());
            s[start..end].to_string()
        };
        assert_eq!(
            options_of(&expected),
            options_of(&contents),
            "man/redis-tui.1 is stale; regenerate it with \
             `cargo run --example generate-man > man/redis-tui.1`"
        );
    }
}

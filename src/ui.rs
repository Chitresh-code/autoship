//! Shared terminal styling and interactivity helpers.
//!
//! `console::Style` already checks terminal support before emitting ANSI codes, so colors
//! disappear automatically on piped output (CI logs, `tests/cli.rs`) without any extra work.

use std::io::IsTerminal;
use std::time::Duration;

use console::Style;
use indicatif::{ProgressBar, ProgressStyle};

/// True when both stdin and stdout are connected to a real terminal.
///
/// `dialoguer` prompts read raw key presses from the controlling terminal and fail outright
/// when piped (`Kind(NotConnected)`), so this gates them: piped or non-interactive runs
/// (including the test suite, which always redirects stdio) fall back to plain, scriptable
/// prompts instead of crashing.
pub fn interactive() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

pub fn success(text: &str) -> String {
    format!(
        "{} {text}",
        Style::new().green().bold().apply_to('\u{2713}')
    )
}

pub fn error(text: &str) -> String {
    format!("{} {text}", Style::new().red().bold().apply_to('\u{2717}'))
}

pub fn heading(text: &str) -> String {
    Style::new().bold().apply_to(text).to_string()
}

pub fn dim(text: &str) -> String {
    Style::new().dim().apply_to(text).to_string()
}

/// A spinner for operations with real, unpredictable latency (network pushes).
///
/// Ticks on its own background thread, so it animates even while the caller is blocked on a
/// synchronous `git` subprocess. `indicatif` detects non-terminal output and draws nothing,
/// so this is safe to call unconditionally, including under `tests/cli.rs`.
pub fn spinner(message: &str) -> ProgressBar {
    let bar = ProgressBar::new_spinner();
    bar.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    bar.set_message(message.to_string());
    bar.enable_steady_tick(Duration::from_millis(80));
    bar
}

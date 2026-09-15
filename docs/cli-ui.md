# CLI UI/UX

Autoship's terminal output is built from a small set of single-purpose crates rather than one all-in-one prompt framework. Each one owns exactly one concern.

* `clap` (already a dependency) generates `--help`/`--version` and parses flags. Its `Command::styles()` hook takes `anstyle` types directly through `clap::builder::styling`, so `--help` is colored without adding `anstyle` as a separate dependency (see `styles()` in `src/main.rs`).
* `console` provides color styling (`ui::success`, `ui::error`, `ui::heading`, `ui::dim` in `src/ui.rs`) and terminal detection. Its styling auto-disables when output isn't a terminal, so piped output (CI logs, `tests/cli.rs`) stays plain text with no stray ANSI codes.
* `indicatif` drives the spinner shown while `git push` is running (the one step with real, unpredictable network latency). It draws nothing when stderr isn't a terminal, so it's safe to call unconditionally.
* `dialoguer` renders the interactive prompts (confirm, select, text input) in `src/confirm.rs` when a real terminal is attached.
* `comfy-table` renders the `--dry-run` plan summary as a bordered key/value table (`src/plan.rs`).

## Two modes, not a TTY-detection fallback

Autoship has exactly two ways to resolve a prompt, matching the PRD's non-interactive mode (section 16):

* **Interactive** (default, attended terminal): every prompt function in `src/confirm.rs` uses `dialoguer` widgets (arrow-key select, confirm, validated text input).
* **`--yes`** (automation): no prompts are shown at all. Each prompt function returns the same default it would have highlighted interactively (accept the suggested version, suggested commit message, suggested branch, the configured/`origin`/first remote, and so on), exactly per the PRD: "This mode should use the generated plan without interactive confirmation."

`dialoguer` (and every other raw-terminal prompt library: `inquire`, `cliclack`) reads key presses directly from the controlling terminal and fails hard (`Kind(NotConnected)`) when stdin isn't a real TTY. So a run that is neither `--yes` nor attached to a terminal, such as CI output being piped with the flag forgotten, cannot be serviced by either mode. `ui::interactive()` (`src/ui.rs`, backed by `std::io::IsTerminal`) detects this, and `confirm::require_interactive()` fails with a clear error telling the user to add `--yes`, rather than silently guessing or blocking forever on empty stdin.

`tests/cli.rs` exercises the real `--yes` automation path end to end (the same one a CI pipeline would use) instead of scripting fake keystrokes through a pipe; the interactive `dialoguer` widgets are verified by hand in a real terminal, since piped stdin cannot drive raw-mode key reads at all.

## Deliberately not used

* `inquire` — overlaps with `dialoguer`; picking one keeps the dependency graph small.
* `tabled` — overlaps with `comfy-table`; the latter is the simpler fit for a handful of ad hoc rows rather than a derived struct collection.
* `owo-colors` / standalone `anstyle` — `console` already covers coloring and is a hard dependency anyway (via `dialoguer`/`indicatif`); a second color crate would be redundant. `anstyle` is still used, but only through clap's own re-export for `--help` styling.
* `miette`, `color-eyre` — CLAUDE.md specifies `anyhow` for application errors. Neither adds value for Autoship's error domain (git/version/plan failures, not source-span diagnostics), and swapping the error backbone is a separate concern from CLI styling.
* `thiserror` — no typed library error types exist in this codebase yet; introducing them isn't part of this UI change.

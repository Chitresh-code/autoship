use anyhow::{Context, Result};
use dialoguer::Confirm as DialoguerConfirm;
use dialoguer::Input as DialoguerInput;
use dialoguer::Select as DialoguerSelect;
use dialoguer::theme::ColorfulTheme;
use semver::Version;

use crate::ui;

fn theme() -> ColorfulTheme {
    ColorfulTheme::default()
}

/// Autoship has exactly two modes: an attended terminal (interactive prompts) or `--yes`
/// (automation, no prompts at all). Anything else, such as piped input with no `--yes`, cannot
/// safely guess an answer and must fail clearly instead (per CLAUDE.md's non-interactive rules).
fn require_interactive() -> Result<()> {
    if ui::interactive() {
        return Ok(());
    }
    anyhow::bail!(
        "not running in an interactive terminal; re-run with --yes for non-interactive automation"
    )
}

/// The user's answer to "Apply version X?".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionChoice {
    Accept,
    Skip,
    Custom(Version),
}

/// Prompts the user to accept, replace, or skip a suggested version bump.
///
/// Never applies a version change silently: the caller always gets an explicit choice back.
/// In `--yes` mode, always accepts the suggested version.
pub fn confirm_version(suggested: &Version, yes: bool) -> Result<VersionChoice> {
    if yes {
        return Ok(VersionChoice::Accept);
    }
    require_interactive()?;

    let options = [
        format!("Apply {suggested}"),
        "Enter a custom version".to_string(),
        "Skip versioning".to_string(),
    ];
    let choice = DialoguerSelect::with_theme(&theme())
        .with_prompt("Version")
        .items(&options)
        .default(0)
        .interact()
        .context("failed to read version choice")?;
    match choice {
        0 => Ok(VersionChoice::Accept),
        1 => {
            let version = DialoguerInput::<String>::with_theme(&theme())
                .with_prompt("Enter version")
                .validate_with(|input: &String| Version::parse(input.trim()).map(|_| ()))
                .interact_text()
                .context("failed to read custom version")?;
            Ok(VersionChoice::Custom(Version::parse(version.trim())?))
        }
        _ => Ok(VersionChoice::Skip),
    }
}

/// Prompts the user to accept a suggested commit message or replace it with their own.
///
/// In `--yes` mode, always accepts the suggested message.
pub fn confirm_commit_message(suggested: &str, yes: bool) -> Result<String> {
    if yes {
        return Ok(suggested.to_string());
    }
    require_interactive()?;

    let use_suggested = DialoguerConfirm::with_theme(&theme())
        .with_prompt("Use this commit message?")
        .default(true)
        .interact()
        .context("failed to read commit message confirmation")?;
    if use_suggested {
        return Ok(suggested.to_string());
    }
    let edited = DialoguerInput::<String>::with_theme(&theme())
        .with_prompt("Commit message")
        .with_initial_text(suggested)
        .validate_with(|input: &String| {
            if input.trim().is_empty() {
                Err("commit message cannot be empty")
            } else {
                Ok(())
            }
        })
        .interact_text()
        .context("failed to read commit message")?;
    Ok(edited.trim().to_string())
}

/// The user's answer to "Where should these changes go?" (PRD section 12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchChoice {
    Current,
    Suggested,
    Custom(String),
}

/// Prompts the user to choose between the current branch, the suggested branch, or a custom
/// branch name.
///
/// In `--yes` mode, always chooses the suggested branch.
pub fn confirm_branch_choice(current: &str, suggested: &str, yes: bool) -> Result<BranchChoice> {
    if yes {
        return Ok(BranchChoice::Suggested);
    }
    require_interactive()?;

    let options = [
        current.to_string(),
        suggested.to_string(),
        "Custom branch".to_string(),
    ];
    let choice = DialoguerSelect::with_theme(&theme())
        .with_prompt("Where should these changes go?")
        .items(&options)
        .default(1)
        .interact()
        .context("failed to read branch choice")?;
    match choice {
        0 => Ok(BranchChoice::Current),
        1 => Ok(BranchChoice::Suggested),
        _ => {
            let name = DialoguerInput::<String>::with_theme(&theme())
                .with_prompt("Branch name")
                .validate_with(|input: &String| {
                    if input.trim().is_empty() {
                        Err("branch name cannot be empty")
                    } else {
                        Ok(())
                    }
                })
                .interact_text()
                .context("failed to read branch name")?;
            Ok(BranchChoice::Custom(name.trim().to_string()))
        }
    }
}

/// Confirms a yes/no decision. In `--yes` mode, returns `default` without prompting.
fn confirm_yes_no(prompt: &str, default: bool, yes: bool) -> Result<bool> {
    if yes {
        return Ok(default);
    }
    require_interactive()?;

    DialoguerConfirm::with_theme(&theme())
        .with_prompt(prompt)
        .default(default)
        .interact()
        .with_context(|| format!("failed to read confirmation for '{prompt}'"))
}

/// Prompts the user to confirm switching to a branch that already exists.
pub fn confirm_switch_to_existing_branch(yes: bool) -> Result<bool> {
    confirm_yes_no("Switch to existing branch?", true, yes)
}

/// Prompts the user to confirm creating a new branch.
pub fn confirm_create_branch(name: &str, yes: bool) -> Result<bool> {
    confirm_yes_no(&format!("Create branch {name}?"), true, yes)
}

/// Prompts the user to confirm committing the staged changes.
pub fn confirm_commit_execution(yes: bool) -> Result<bool> {
    confirm_yes_no("Commit staged changes with this message?", true, yes)
}

/// The user's answer to "Push to:" when multiple remotes exist (PRD section 13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteChoice {
    Named(String),
    Custom(String),
}

/// Prompts the user to choose which remote to push to, when multiple remotes exist.
///
/// `default` is the remote Autoship would pick on its own (the configured preferred remote,
/// else `origin`, else the first one); it's highlighted by default in the interactive picker
/// and chosen outright in `--yes` mode.
pub fn confirm_remote_choice(remotes: &[String], default: &str, yes: bool) -> Result<RemoteChoice> {
    if yes {
        return Ok(RemoteChoice::Named(default.to_string()));
    }
    require_interactive()?;

    let default_index = remotes.iter().position(|r| r == default).unwrap_or(0);
    let mut options: Vec<String> = remotes.to_vec();
    options.push("Custom remote".to_string());
    let choice = DialoguerSelect::with_theme(&theme())
        .with_prompt("Push to")
        .items(&options)
        .default(default_index)
        .interact()
        .context("failed to read remote choice")?;
    if choice < remotes.len() {
        Ok(RemoteChoice::Named(remotes[choice].clone()))
    } else {
        let name = DialoguerInput::<String>::with_theme(&theme())
            .with_prompt("Remote name")
            .validate_with(|input: &String| {
                if input.trim().is_empty() {
                    Err("remote name cannot be empty")
                } else {
                    Ok(())
                }
            })
            .interact_text()
            .context("failed to read remote name")?;
        Ok(RemoteChoice::Custom(name.trim().to_string()))
    }
}

/// Prompts the user to confirm using the single detected remote.
pub fn confirm_use_remote(name: &str, yes: bool) -> Result<bool> {
    confirm_yes_no(&format!("Use {name}?"), true, yes)
}

/// Prompts the user to confirm pushing a branch to a remote.
pub fn confirm_push(remote: &str, branch: &str, yes: bool) -> Result<bool> {
    confirm_yes_no(&format!("Push {branch} to {remote}?"), true, yes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yes_mode_returns_the_default_without_prompting() {
        assert!(confirm_yes_no("prompt", true, true).unwrap());
        assert!(!confirm_yes_no("prompt", false, true).unwrap());
    }

    #[test]
    fn yes_mode_accepts_the_suggested_version() {
        let suggested = Version::new(1, 2, 3);
        assert_eq!(
            confirm_version(&suggested, true).unwrap(),
            VersionChoice::Accept
        );
    }

    #[test]
    fn yes_mode_accepts_the_suggested_commit_message() {
        assert_eq!(
            confirm_commit_message("feat: thing", true).unwrap(),
            "feat: thing"
        );
    }

    #[test]
    fn yes_mode_chooses_the_suggested_branch() {
        assert_eq!(
            confirm_branch_choice("main", "feature/x", true).unwrap(),
            BranchChoice::Suggested
        );
    }

    #[test]
    fn yes_mode_chooses_the_default_remote() {
        let remotes = vec!["origin".to_string(), "upstream".to_string()];
        assert_eq!(
            confirm_remote_choice(&remotes, "upstream", true).unwrap(),
            RemoteChoice::Named("upstream".to_string())
        );
    }

    /// `cargo test` never attaches a real terminal, so this exercises the real "neither
    /// interactive nor --yes" error path without needing a subprocess.
    #[test]
    fn errors_clearly_when_neither_interactive_nor_yes() {
        let suggested = Version::new(1, 0, 0);
        let err = confirm_version(&suggested, false).unwrap_err();
        assert!(err.to_string().contains("--yes"));
    }
}

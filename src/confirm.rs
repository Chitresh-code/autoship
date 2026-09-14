use std::io::{self, Write};

use anyhow::{Context, Result};
use semver::Version;

/// The user's answer to "Apply version X?".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionChoice {
    Accept,
    Skip,
    Custom(Version),
}

/// Interprets a single line of prompt input.
///
/// Returns `Ok(None)` when the user typed the bare word `custom`, meaning the caller should
/// prompt again for the actual version. Typing a version directly (e.g. `2.0.0`) is also
/// accepted as shorthand, skipping that second prompt.
fn parse_version_response(input: &str) -> Result<Option<VersionChoice>> {
    let trimmed = input.trim();
    match trimmed.to_lowercase().as_str() {
        "" | "y" | "yes" => Ok(Some(VersionChoice::Accept)),
        "n" | "no" | "skip" => Ok(Some(VersionChoice::Skip)),
        "custom" => Ok(None),
        _ => {
            let version = Version::parse(trimmed).with_context(|| {
                format!(
                    "unrecognized response '{trimmed}': expected y, n, custom, skip, or a version"
                )
            })?;
            Ok(Some(VersionChoice::Custom(version)))
        }
    }
}

fn read_line(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout().flush().context("failed to write to stdout")?;
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .context("failed to read from stdin")?;
    Ok(line)
}

/// Prompts the user to accept, replace, or skip a suggested version bump.
///
/// Never applies a version change silently: the caller always gets an explicit choice back.
pub fn confirm_version(suggested: &Version) -> Result<VersionChoice> {
    let answer = read_line(&format!("? Apply version {suggested}? [Y/n/custom/skip] "))?;
    match parse_version_response(&answer)? {
        Some(choice) => Ok(choice),
        None => {
            let custom = read_line("Enter version: ")?;
            let version = Version::parse(custom.trim())
                .with_context(|| format!("invalid version '{}'", custom.trim()))?;
            Ok(VersionChoice::Custom(version))
        }
    }
}

/// Interprets a single line of response to "Use this message?".
fn parse_commit_response(input: &str) -> Result<bool> {
    match input.trim().to_lowercase().as_str() {
        "" | "y" | "yes" => Ok(true),
        "n" | "no" | "edit" => Ok(false),
        other => anyhow::bail!("unrecognized response '{other}': expected y, n, or edit"),
    }
}

/// Prompts the user to accept a suggested commit message or replace it with their own.
pub fn confirm_commit_message(suggested: &str) -> Result<String> {
    let answer = read_line("? Use this message? [Y/n/edit] ")?;
    if parse_commit_response(&answer)? {
        return Ok(suggested.to_string());
    }
    let edited = read_line("Commit message: ")?;
    let trimmed = edited.trim();
    if trimmed.is_empty() {
        anyhow::bail!("commit message cannot be empty");
    }
    Ok(trimmed.to_string())
}

/// The user's answer to "Where should these changes go?" (PRD section 12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchChoice {
    Current,
    Suggested,
    Custom(String),
}

/// Interprets a single line of response to the numbered branch-strategy prompt.
///
/// Returns `Ok(None)` for choice 3 (custom branch), meaning the caller should prompt again
/// for the actual branch name.
fn parse_branch_choice(input: &str) -> Result<Option<BranchChoice>> {
    match input.trim() {
        "1" => Ok(Some(BranchChoice::Current)),
        "2" => Ok(Some(BranchChoice::Suggested)),
        "3" => Ok(None),
        other => anyhow::bail!("unrecognized response '{other}': expected 1, 2, or 3"),
    }
}

/// Prompts the user to choose between the current branch, the suggested branch, or a custom
/// branch name.
pub fn confirm_branch_choice() -> Result<BranchChoice> {
    let answer = read_line("> ")?;
    match parse_branch_choice(&answer)? {
        Some(choice) => Ok(choice),
        None => {
            let name = read_line("Branch name: ")?;
            let trimmed = name.trim();
            if trimmed.is_empty() {
                anyhow::bail!("branch name cannot be empty");
            }
            Ok(BranchChoice::Custom(trimmed.to_string()))
        }
    }
}

/// Interprets a single line of yes/no response.
fn parse_yes_no(input: &str) -> Result<bool> {
    match input.trim().to_lowercase().as_str() {
        "" | "y" | "yes" => Ok(true),
        "n" | "no" => Ok(false),
        other => anyhow::bail!("unrecognized response '{other}': expected y or n"),
    }
}

/// Prompts the user to confirm switching to a branch that already exists.
pub fn confirm_switch_to_existing_branch() -> Result<bool> {
    let answer = read_line("? Switch to existing branch? [Y/n] ")?;
    parse_yes_no(&answer)
}

/// Prompts the user to confirm creating a new branch.
pub fn confirm_create_branch(name: &str) -> Result<bool> {
    let answer = read_line(&format!("? Create branch {name}? [Y/n] "))?;
    parse_yes_no(&answer)
}

/// Prompts the user to confirm committing the staged changes.
pub fn confirm_commit_execution() -> Result<bool> {
    let answer = read_line("? Commit staged changes with this message? [Y/n] ")?;
    parse_yes_no(&answer)
}

/// The user's answer to "Push to:" when multiple remotes exist (PRD section 13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteChoice {
    Named(String),
    Custom(String),
}

/// Interprets a numbered response to the multi-remote "Push to:" prompt.
///
/// `remote_count` is how many named remotes were listed; the option after the last one is
/// always "Custom remote", which returns `Ok(None)` deferring to a second prompt.
fn parse_remote_index(input: &str, remote_count: usize) -> Result<Option<usize>> {
    let trimmed = input.trim();
    let choice: usize = trimmed
        .parse()
        .map_err(|_| anyhow::anyhow!("unrecognized response '{trimmed}': expected a number"))?;
    if choice == 0 || choice > remote_count + 1 {
        anyhow::bail!(
            "unrecognized response '{trimmed}': expected 1-{}",
            remote_count + 1
        );
    }
    Ok((choice <= remote_count).then_some(choice - 1))
}

/// Prompts the user to choose which remote to push to, when multiple remotes exist.
pub fn confirm_remote_choice(remotes: &[String]) -> Result<RemoteChoice> {
    let answer = read_line("> ")?;
    match parse_remote_index(&answer, remotes.len())? {
        Some(index) => Ok(RemoteChoice::Named(remotes[index].clone())),
        None => {
            let name = read_line("Remote name: ")?;
            let trimmed = name.trim();
            if trimmed.is_empty() {
                anyhow::bail!("remote name cannot be empty");
            }
            Ok(RemoteChoice::Custom(trimmed.to_string()))
        }
    }
}

/// Prompts the user to confirm using the single detected remote.
pub fn confirm_use_remote(name: &str) -> Result<bool> {
    let answer = read_line(&format!("Use {name}? [Y/n] "))?;
    parse_yes_no(&answer)
}

/// Prompts the user to confirm pushing a branch to a remote.
pub fn confirm_push(remote: &str, branch: &str) -> Result<bool> {
    let answer = read_line(&format!("? Push {branch} to {remote}? [Y/n] "))?;
    parse_yes_no(&answer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_default_and_yes_responses() {
        for input in ["", "y", "Y", "yes"] {
            assert_eq!(
                parse_version_response(input).unwrap(),
                Some(VersionChoice::Accept)
            );
        }
    }

    #[test]
    fn skips_on_no_and_skip_responses() {
        for input in ["n", "no", "skip"] {
            assert_eq!(
                parse_version_response(input).unwrap(),
                Some(VersionChoice::Skip)
            );
        }
    }

    #[test]
    fn custom_keyword_defers_to_a_second_prompt() {
        assert_eq!(parse_version_response("custom").unwrap(), None);
    }

    #[test]
    fn a_bare_version_is_accepted_as_custom_shorthand() {
        assert_eq!(
            parse_version_response("2.0.0").unwrap(),
            Some(VersionChoice::Custom(Version::new(2, 0, 0)))
        );
    }

    #[test]
    fn rejects_unrecognized_input() {
        assert!(parse_version_response("maybe").is_err());
    }

    #[test]
    fn commit_response_accepts_default_and_yes() {
        for input in ["", "y", "Y", "yes"] {
            assert!(parse_commit_response(input).unwrap());
        }
    }

    #[test]
    fn commit_response_declines_on_no_and_edit() {
        for input in ["n", "no", "edit"] {
            assert!(!parse_commit_response(input).unwrap());
        }
    }

    #[test]
    fn commit_response_rejects_unrecognized_input() {
        assert!(parse_commit_response("maybe").is_err());
    }

    #[test]
    fn branch_choice_one_selects_current() {
        assert_eq!(
            parse_branch_choice("1").unwrap(),
            Some(BranchChoice::Current)
        );
    }

    #[test]
    fn branch_choice_two_selects_suggested() {
        assert_eq!(
            parse_branch_choice("2").unwrap(),
            Some(BranchChoice::Suggested)
        );
    }

    #[test]
    fn branch_choice_three_defers_to_a_second_prompt() {
        assert_eq!(parse_branch_choice("3").unwrap(), None);
    }

    #[test]
    fn branch_choice_rejects_unrecognized_input() {
        assert!(parse_branch_choice("4").is_err());
    }

    #[test]
    fn yes_no_accepts_default_and_yes_responses() {
        for input in ["", "y", "Y", "yes"] {
            assert!(parse_yes_no(input).unwrap());
        }
    }

    #[test]
    fn yes_no_declines_on_no_responses() {
        for input in ["n", "no"] {
            assert!(!parse_yes_no(input).unwrap());
        }
    }

    #[test]
    fn yes_no_rejects_unrecognized_input() {
        assert!(parse_yes_no("maybe").is_err());
    }

    #[test]
    fn remote_index_selects_a_named_remote() {
        assert_eq!(parse_remote_index("1", 2).unwrap(), Some(0));
        assert_eq!(parse_remote_index("2", 2).unwrap(), Some(1));
    }

    #[test]
    fn remote_index_defers_to_a_second_prompt_for_the_last_option() {
        assert_eq!(parse_remote_index("3", 2).unwrap(), None);
    }

    #[test]
    fn remote_index_rejects_out_of_range_and_non_numeric_input() {
        assert!(parse_remote_index("0", 2).is_err());
        assert!(parse_remote_index("4", 2).is_err());
        assert!(parse_remote_index("abc", 2).is_err());
    }
}

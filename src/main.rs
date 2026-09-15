mod branch;
mod bump;
mod commit;
mod config;
mod confirm;
mod git;
mod plan;
mod ui;
mod version;

use std::process::ExitCode;

use clap::Parser;
use clap::builder::styling::{AnsiColor, Styles};

fn styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Cyan.on_default().bold())
        .usage(AnsiColor::Cyan.on_default().bold())
        .literal(AnsiColor::Green.on_default().bold())
        .placeholder(AnsiColor::Cyan.on_default())
}

/// Autoship: automate the workflow from staged changes to pushed code.
#[derive(Parser)]
#[command(version, about, styles = styles())]
struct Cli {
    /// Show the planned actions without changing files, branches, commits, or remote state.
    #[arg(long)]
    dry_run: bool,

    /// Accept the generated plan without interactive confirmation, for automation.
    #[arg(long)]
    yes: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Err(err) = run(cli.dry_run, cli.yes) {
        eprintln!("{}", ui::error(&format!("Error: {err:#}")));
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// The remote `autoship` targets by default when the user isn't asked to choose: the configured
/// preferred remote when it exists, else `origin`, else the first configured remote.
fn default_remote<'a>(remotes: &'a [(String, String)], preferred: Option<&str>) -> Option<&'a str> {
    preferred
        .and_then(|preferred| remotes.iter().find(|(name, _)| name == preferred))
        .or_else(|| remotes.iter().find(|(name, _)| name == "origin"))
        .or_else(|| remotes.first())
        .map(|(name, _)| name.as_str())
}

fn run(dry_run: bool, yes: bool) -> anyhow::Result<()> {
    if !git::is_repository() {
        println!("Not a Git repository.");
        return Ok(());
    }
    println!("{}", ui::success("Git repository detected"));
    let branch = git::current_branch()?;
    println!("{}", ui::success(&format!("On branch {branch}")));

    let staged = git::staged_files()?;
    let unstaged = git::unstaged_files()?;

    if staged.is_empty() {
        println!();
        println!("No staged changes found.");
        println!();
        println!("Stage your changes first:");
        println!();
        println!("  git add <files>");
        println!();
        println!("Then run:");
        println!();
        println!("  autoship");
        return Ok(());
    }
    println!(
        "{}",
        ui::success(&format!(
            "{} staged file{}",
            staged.len(),
            if staged.len() == 1 { "" } else { "s" }
        ))
    );

    let remotes = git::remotes()?;
    for (name, _) in &remotes {
        println!("{}", ui::success(&format!("{name} remote detected")));
    }

    let repo_root = git::repository_root()?;
    let config = config::load(&repo_root)?;
    let detected_version = version::detect(&repo_root)?;
    if let Some(v) = &detected_version {
        println!(
            "{}",
            ui::success(&format!("{} project detected", v.ecosystem))
        );
    }

    println!();
    println!("{}", ui::heading("Changes"));
    println!();
    for file in &staged {
        let marker = file.status.marker();
        let colored = match file.status {
            git::FileStatus::Added => console::style(marker).green(),
            git::FileStatus::Modified => console::style(marker).yellow(),
            git::FileStatus::Deleted => console::style(marker).red(),
            git::FileStatus::Renamed => console::style(marker).cyan(),
            git::FileStatus::Other => console::style(marker).dim(),
        };
        println!("  {colored} {}", file.path);
    }

    let classification = bump::classify(&staged);
    let version_change = detected_version.as_ref().and_then(|v| {
        let change = classification.bump?;
        let current = semver::Version::parse(&v.version).ok()?;
        Some((v.version.clone(), change.apply(&current).to_string()))
    });
    let suggested_commit = commit::suggest(&staged, &classification);
    let suggested_branch = branch::suggest(&suggested_commit, &config.branch_prefixes);

    if dry_run {
        println!();
        print!(
            "{}",
            plan::render(
                version_change
                    .as_ref()
                    .map(|(c, s)| (c.as_str(), s.as_str())),
                (branch.as_str(), suggested_branch.as_str()),
                &suggested_commit.to_string(),
                default_remote(&remotes, config.remote.as_deref()),
            )
        );
        return Ok(());
    }

    if let Some(v) = &detected_version {
        println!();
        println!("Current version: {}", v.version);
        match (classification.bump, semver::Version::parse(&v.version)) {
            (Some(change), Ok(current)) => {
                let suggested = change.apply(&current);
                println!("Suggested version: {suggested} ({})", change.label());
                println!();
                match confirm::confirm_version(&suggested, yes)? {
                    confirm::VersionChoice::Accept => {
                        version::apply(v, &suggested.to_string())?;
                        println!(
                            "{}",
                            ui::success(&format!("Version updated to {suggested}"))
                        );
                    }
                    confirm::VersionChoice::Custom(custom) => {
                        version::apply(v, &custom.to_string())?;
                        println!("{}", ui::success(&format!("Version updated to {custom}")));
                    }
                    confirm::VersionChoice::Skip => {
                        println!("{}", ui::dim("Versioning skipped."));
                    }
                }
            }
            (None, _) => {
                println!(
                    "Suggested version: none ({} change)",
                    classification.kind.label()
                );
            }
            (Some(_), Err(_)) => {
                println!("Suggested version: none (unrecognized version format, skipping)");
            }
        }
    }

    println!();
    println!("Suggested commit:");
    println!();
    println!("{suggested_commit}");
    println!();
    let commit_message = confirm::confirm_commit_message(&suggested_commit.to_string(), yes)?;
    println!();
    println!("Commit message: {commit_message}");

    println!();
    println!("Suggested branch: {suggested_branch}");

    println!();
    println!("Current branch:");
    println!("  {branch}");
    println!();
    let target_branch = match confirm::confirm_branch_choice(&branch, &suggested_branch, yes)? {
        confirm::BranchChoice::Current => branch.clone(),
        confirm::BranchChoice::Suggested => suggested_branch.clone(),
        confirm::BranchChoice::Custom(name) => name,
    };
    println!();
    println!("Branch: {target_branch}");

    if target_branch != branch {
        if git::branch_exists(&target_branch)? {
            println!();
            println!("Branch already exists:");
            println!();
            println!("{target_branch}");
            println!();
            if confirm::confirm_switch_to_existing_branch(yes)? {
                git::switch_branch(&target_branch)?;
                println!(
                    "{}",
                    ui::success(&format!("Switched to branch {target_branch}"))
                );
            } else {
                println!(
                    "{}",
                    ui::dim(&format!("Staying on current branch: {branch}"))
                );
            }
        } else if confirm::confirm_create_branch(&target_branch, yes)? {
            git::create_branch(&target_branch)?;
            println!(
                "{}",
                ui::success(&format!("Branch created: {target_branch}"))
            );
        } else {
            println!("{}", ui::dim("Branch not created."));
        }
    }
    let active_branch = git::current_branch()?;

    println!();
    if confirm::confirm_commit_execution(yes)? {
        git::commit(&commit_message)?;
        println!("{}", ui::success("Changes committed"));
    } else {
        println!("{}", ui::dim("Commit skipped."));
    }

    println!();
    let selected_remote = if remotes.is_empty() {
        println!("No Git remote configured.");
        None
    } else {
        let remote_names: Vec<String> = remotes.iter().map(|(name, _)| name.clone()).collect();
        let selected = if let [only] = remote_names.as_slice() {
            println!("Remote:");
            println!("  {only}");
            println!();
            confirm::confirm_use_remote(only, yes)?.then(|| only.clone())
        } else {
            let default = default_remote(&remotes, config.remote.as_deref())
                .unwrap_or(remote_names[0].as_str());
            Some(
                match confirm::confirm_remote_choice(&remote_names, default, yes)? {
                    confirm::RemoteChoice::Named(name) | confirm::RemoteChoice::Custom(name) => {
                        name
                    }
                },
            )
        };
        println!();
        match &selected {
            Some(name) => println!("Remote selected: {name}"),
            None => println!("{}", ui::dim("No remote selected.")),
        }
        selected
    };

    if let Some(remote) = selected_remote {
        println!();
        if confirm::confirm_push(&remote, &active_branch, yes)? {
            let set_upstream = !git::has_upstream(&active_branch)?;
            let spinner = ui::spinner(&format!("Pushing to {remote}..."));
            let result = git::push(&remote, &active_branch, set_upstream);
            spinner.finish_and_clear();
            result?;
            println!(
                "{}",
                ui::success(&format!("Pushed {remote}/{active_branch}"))
            );
            if set_upstream {
                println!("{}", ui::success("Upstream configured"));
            }
        } else {
            println!("{}", ui::dim("Push skipped."));
        }
    }

    if !unstaged.is_empty() {
        println!();
        println!("You have unstaged changes.");
        println!();
        println!("Staged:");
        for file in &staged {
            println!("  {}", file.path);
        }
        println!();
        println!("Unstaged:");
        for file in &unstaged {
            println!("  {file}");
        }
        println!();
        println!("Only staged changes will be included.");
    }

    Ok(())
}

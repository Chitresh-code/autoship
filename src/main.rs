mod branch;
mod bump;
mod commit;
mod config;
mod confirm;
mod git;
mod plan;
mod version;

use std::process::ExitCode;

use clap::Parser;

/// Autoship: automate the workflow from staged changes to pushed code.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Show the planned actions without changing files, branches, commits, or remote state.
    #[arg(long)]
    dry_run: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Err(err) = run(cli.dry_run) {
        eprintln!("Error: {err:#}");
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

fn run(dry_run: bool) -> anyhow::Result<()> {
    if !git::is_repository() {
        println!("Not a Git repository.");
        return Ok(());
    }
    println!("✓ Git repository detected");
    let branch = git::current_branch()?;
    println!("✓ On branch {branch}");

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
        "✓ {} staged file{}",
        staged.len(),
        if staged.len() == 1 { "" } else { "s" }
    );

    let remotes = git::remotes()?;
    for (name, _) in &remotes {
        println!("✓ {name} remote detected");
    }

    let repo_root = git::repository_root()?;
    let config = config::load(&repo_root)?;
    let detected_version = version::detect(&repo_root)?;
    if let Some(v) = &detected_version {
        println!("✓ {} project detected", v.ecosystem);
    }

    println!();
    println!("Changes");
    println!();
    for file in &staged {
        println!("  {} {}", file.status.marker(), file.path);
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
                match confirm::confirm_version(&suggested)? {
                    confirm::VersionChoice::Accept => {
                        println!("Version confirmed: {suggested}");
                    }
                    confirm::VersionChoice::Custom(custom) => {
                        println!("Version set to: {custom}");
                    }
                    confirm::VersionChoice::Skip => {
                        println!("Versioning skipped.");
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
    let commit_message = confirm::confirm_commit_message(&suggested_commit.to_string())?;
    println!();
    println!("Commit message: {commit_message}");

    println!();
    println!("Suggested branch: {suggested_branch}");

    println!();
    println!("Current branch:");
    println!("  {branch}");
    println!();
    println!("? Where should these changes go?");
    println!();
    println!("  1. {branch}");
    println!("  2. {suggested_branch}");
    println!("  3. Custom branch");
    println!();
    let target_branch = match confirm::confirm_branch_choice()? {
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
            if confirm::confirm_switch_to_existing_branch()? {
                git::switch_branch(&target_branch)?;
                println!("✓ Switched to branch {target_branch}");
            } else {
                println!("Staying on current branch: {branch}");
            }
        } else if confirm::confirm_create_branch(&target_branch)? {
            git::create_branch(&target_branch)?;
            println!("✓ Branch created: {target_branch}");
        } else {
            println!("Branch not created.");
        }
    }
    let active_branch = git::current_branch()?;

    println!();
    if confirm::confirm_commit_execution()? {
        git::commit(&commit_message)?;
        println!("✓ Changes committed");
    } else {
        println!("Commit skipped.");
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
            confirm::confirm_use_remote(only)?.then(|| only.clone())
        } else {
            println!("? Push to:");
            println!();
            for (i, name) in remote_names.iter().enumerate() {
                println!("  {}. {name}", i + 1);
            }
            println!("  {}. Custom remote", remote_names.len() + 1);
            println!();
            Some(match confirm::confirm_remote_choice(&remote_names)? {
                confirm::RemoteChoice::Named(name) | confirm::RemoteChoice::Custom(name) => name,
            })
        };
        println!();
        match &selected {
            Some(name) => println!("Remote selected: {name}"),
            None => println!("No remote selected."),
        }
        selected
    };

    if let Some(remote) = selected_remote {
        println!();
        if confirm::confirm_push(&remote, &active_branch)? {
            let set_upstream = !git::has_upstream(&active_branch)?;
            git::push(&remote, &active_branch, set_upstream)?;
            println!("✓ Pushed {remote}/{active_branch}");
            if set_upstream {
                println!("✓ Upstream configured");
            }
        } else {
            println!("Push skipped.");
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

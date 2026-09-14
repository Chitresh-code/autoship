use std::process::Command;

use anyhow::{Context, Result, bail};

/// Runs `git <args>` in the current directory and returns trimmed stdout.
fn run_git(args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .with_context(|| format!("failed to execute `git {}`", args.join(" ")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("`git {}` failed: {}", args.join(" "), stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// True if the current directory is inside a Git work tree.
pub fn is_repository() -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Current branch name, or the short commit hash when HEAD is detached.
pub fn current_branch() -> Result<String> {
    // `symbolic-ref` fails (expected, not a bug) when HEAD is detached; fall
    // back to the short commit hash in that case.
    let branch = run_git(["symbolic-ref", "--short", "-q", "HEAD"].as_slice()).unwrap_or_default();
    if !branch.is_empty() {
        return Ok(branch);
    }
    run_git(["rev-parse", "--short", "HEAD"].as_slice())
}

fn name_only_lines(output: String) -> Vec<String> {
    output
        .lines()
        .map(str::to_string)
        .filter(|l| !l.is_empty())
        .collect()
}

/// How a staged file differs from `HEAD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Other,
}

impl FileStatus {
    /// Single-letter marker for CLI output, matching `git status` conventions.
    pub fn marker(self) -> char {
        match self {
            FileStatus::Added => 'A',
            FileStatus::Modified => 'M',
            FileStatus::Deleted => 'D',
            FileStatus::Renamed => 'R',
            FileStatus::Other => '?',
        }
    }
}

/// A file staged for commit, with its change status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedFile {
    pub path: String,
    pub status: FileStatus,
}

/// Files staged for commit (index vs HEAD), with their change status.
pub fn staged_files() -> Result<Vec<StagedFile>> {
    let out = run_git(["diff", "--cached", "--name-status"].as_slice())?;
    Ok(out
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let code = fields.next()?;
            // Renames/copies are "R100\told\tnew"; the path we care about is the last field.
            let path = fields.next_back()?;
            let status = match code.as_bytes().first()? {
                b'A' => FileStatus::Added,
                b'M' => FileStatus::Modified,
                b'D' => FileStatus::Deleted,
                b'R' | b'C' => FileStatus::Renamed,
                _ => FileStatus::Other,
            };
            Some(StagedFile {
                path: path.to_string(),
                status,
            })
        })
        .collect())
}

/// Tracked files with unstaged modifications (working tree vs index).
pub fn unstaged_files() -> Result<Vec<String>> {
    let out = run_git(["diff", "--name-only"].as_slice())?;
    Ok(name_only_lines(out))
}

/// The repository's top-level working directory.
pub fn repository_root() -> Result<std::path::PathBuf> {
    let root = run_git(["rev-parse", "--show-toplevel"].as_slice())?;
    Ok(std::path::PathBuf::from(root))
}

/// True if a local branch with this name exists.
pub fn branch_exists(name: &str) -> Result<bool> {
    let status = Command::new("git")
        .args([
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{name}"),
        ])
        .status()
        .with_context(|| format!("failed to check whether branch '{name}' exists"))?;
    Ok(status.success())
}

/// Creates a new branch from the current `HEAD` and switches to it.
pub fn create_branch(name: &str) -> Result<()> {
    run_git(["checkout", "-b", name].as_slice())?;
    Ok(())
}

/// Switches to an existing branch.
pub fn switch_branch(name: &str) -> Result<()> {
    run_git(["checkout", name].as_slice())?;
    Ok(())
}

/// Commits the staged changes with the given message.
pub fn commit(message: &str) -> Result<()> {
    run_git(["commit", "-m", message].as_slice())?;
    Ok(())
}

/// True if the given local branch has an upstream tracking branch configured.
pub fn has_upstream(branch: &str) -> Result<bool> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", &format!("{branch}@{{u}}")])
        .output()
        .with_context(|| format!("failed to check upstream for branch '{branch}'"))?;
    Ok(output.status.success())
}

/// Pushes `branch` to `remote`, configuring upstream tracking when `set_upstream` is true.
pub fn push(remote: &str, branch: &str, set_upstream: bool) -> Result<()> {
    let mut args = vec!["push"];
    if set_upstream {
        args.push("-u");
    }
    args.push(remote);
    args.push(branch);
    run_git(&args)?;
    Ok(())
}

/// Configured remotes as (name, fetch url), deduplicated and in `git remote -v` order.
pub fn remotes() -> Result<Vec<(String, String)>> {
    let out = run_git(["remote", "-v"].as_slice())?;
    let mut remotes = Vec::new();
    for line in out.lines() {
        let Some((name, rest)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        if !rest.trim_end().ends_with("(fetch)") {
            continue;
        }
        let url = rest.trim().trim_end_matches("(fetch)").trim();
        remotes.push((name.to_string(), url.to_string()));
    }
    Ok(remotes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempRepo {
        dir: std::path::PathBuf,
    }

    impl TempRepo {
        fn new() -> Self {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("ship-git-test-{nanos}-{n}"));
            fs::create_dir_all(&dir).unwrap();
            let status = Command::new("git")
                .args(["init", "-q"])
                .current_dir(&dir)
                .status()
                .unwrap();
            assert!(status.success());
            for (key, value) in [("user.email", "test@example.com"), ("user.name", "Test")] {
                Command::new("git")
                    .args(["config", key, value])
                    .current_dir(&dir)
                    .status()
                    .unwrap();
            }
            Self { dir }
        }

        fn write(&self, name: &str, contents: &str) {
            fs::write(self.dir.join(name), contents).unwrap();
        }

        fn git(&self, args: &[&str]) {
            let status = Command::new("git")
                .args(args)
                .current_dir(&self.dir)
                .status()
                .unwrap();
            assert!(status.success());
        }

        fn enter(&self) -> DirGuard {
            let previous = std::env::current_dir().unwrap();
            std::env::set_current_dir(&self.dir).unwrap();
            DirGuard { previous }
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    struct DirGuard {
        previous: std::path::PathBuf,
    }

    impl Drop for DirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.previous);
        }
    }

    // Git process-spawning tests share the current working directory, so they
    // must not run concurrently with each other.
    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn detects_staged_and_unstaged_changes() {
        let _guard = TEST_LOCK.lock().unwrap();
        let repo = TempRepo::new();
        let _dir = repo.enter();

        assert!(is_repository());

        repo.write("committed.txt", "initial\n");
        repo.git(&["add", "committed.txt"]);
        repo.git(&["commit", "-q", "-m", "initial"]);

        repo.write("staged.txt", "staged\n");
        repo.git(&["add", "staged.txt"]);
        repo.write("committed.txt", "changed\n");

        let staged = staged_files().unwrap();
        let unstaged = unstaged_files().unwrap();

        assert_eq!(
            staged,
            vec![StagedFile {
                path: "staged.txt".to_string(),
                status: FileStatus::Added,
            }]
        );
        assert_eq!(unstaged, vec!["committed.txt".to_string()]);
    }

    #[test]
    fn reports_no_repository_outside_git() {
        let _guard = TEST_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!(
            "ship-not-a-repo-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let result = is_repository();

        std::env::set_current_dir(&previous).unwrap();
        fs::remove_dir_all(&dir).unwrap();

        assert!(!result);
    }

    #[test]
    fn detects_whether_a_branch_exists() {
        let _guard = TEST_LOCK.lock().unwrap();
        let repo = TempRepo::new();
        let _dir = repo.enter();
        repo.write("committed.txt", "initial\n");
        repo.git(&["add", "committed.txt"]);
        repo.git(&["commit", "-q", "-m", "initial"]);
        repo.git(&["branch", "feature/exists"]);

        assert!(branch_exists("feature/exists").unwrap());
        assert!(!branch_exists("feature/does-not-exist").unwrap());
    }

    #[test]
    fn creates_and_switches_to_a_new_branch() {
        let _guard = TEST_LOCK.lock().unwrap();
        let repo = TempRepo::new();
        let _dir = repo.enter();
        repo.write("committed.txt", "initial\n");
        repo.git(&["add", "committed.txt"]);
        repo.git(&["commit", "-q", "-m", "initial"]);

        create_branch("feature/new-thing").unwrap();

        assert_eq!(current_branch().unwrap(), "feature/new-thing");
    }

    #[test]
    fn switches_to_an_existing_branch() {
        let _guard = TEST_LOCK.lock().unwrap();
        let repo = TempRepo::new();
        let _dir = repo.enter();
        repo.write("committed.txt", "initial\n");
        repo.git(&["add", "committed.txt"]);
        repo.git(&["commit", "-q", "-m", "initial"]);
        repo.git(&["branch", "other"]);

        switch_branch("other").unwrap();

        assert_eq!(current_branch().unwrap(), "other");
    }

    #[test]
    fn commits_staged_changes() {
        let _guard = TEST_LOCK.lock().unwrap();
        let repo = TempRepo::new();
        let _dir = repo.enter();
        repo.write("committed.txt", "initial\n");
        repo.git(&["add", "committed.txt"]);
        repo.git(&["commit", "-q", "-m", "initial"]);
        repo.write("staged.txt", "staged\n");
        repo.git(&["add", "staged.txt"]);

        commit("feat: add staged file").unwrap();

        assert!(staged_files().unwrap().is_empty());
        let log = Command::new("git")
            .args(["log", "-1", "--format=%s"])
            .current_dir(&repo.dir)
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&log.stdout).trim(),
            "feat: add staged file"
        );
    }

    #[test]
    fn pushes_a_new_branch_and_configures_upstream() {
        let _guard = TEST_LOCK.lock().unwrap();
        let repo = TempRepo::new();
        let remote_dir = std::env::temp_dir().join(format!(
            "ship-git-test-remote-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&remote_dir).unwrap();
        assert!(
            Command::new("git")
                .args(["init", "-q", "--bare"])
                .current_dir(&remote_dir)
                .status()
                .unwrap()
                .success()
        );

        let _dir = repo.enter();
        repo.write("committed.txt", "initial\n");
        repo.git(&["add", "committed.txt"]);
        repo.git(&["commit", "-q", "-m", "initial"]);
        repo.git(&["remote", "add", "origin", remote_dir.to_str().unwrap()]);

        let branch = current_branch().unwrap();
        assert!(!has_upstream(&branch).unwrap());

        push("origin", &branch, true).unwrap();
        assert!(has_upstream(&branch).unwrap());

        repo.write("committed.txt", "changed\n");
        repo.git(&["commit", "-q", "-am", "second"]);
        push("origin", &branch, false).unwrap();

        fs::remove_dir_all(&remote_dir).unwrap();
    }

    #[test]
    fn parses_remotes() {
        let _guard = TEST_LOCK.lock().unwrap();
        let repo = TempRepo::new();
        let _dir = repo.enter();
        repo.git(&["remote", "add", "origin", "git@example.com:user/repo.git"]);

        let remotes = remotes().unwrap();

        assert_eq!(
            remotes,
            vec![(
                "origin".to_string(),
                "git@example.com:user/repo.git".to_string()
            )]
        );
    }
}

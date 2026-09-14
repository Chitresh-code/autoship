//! End-to-end tests that run the actual `autoship` binary against disposable Git repositories
//! (never the developer's own repository), per CLAUDE.md's Git safety and testing rules.

use std::fs;
use std::path::PathBuf;
use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use assert_cmd::Command;

struct TestRepo {
    dir: PathBuf,
}

impl TestRepo {
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("autoship-cli-test-{nanos}-{n}"));
        fs::create_dir_all(&dir).unwrap();
        assert!(
            StdCommand::new("git")
                .args(["init", "-q", "-b", "main"])
                .current_dir(&dir)
                .status()
                .unwrap()
                .success()
        );
        for (key, value) in [("user.email", "test@example.com"), ("user.name", "Test")] {
            StdCommand::new("git")
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
        assert!(
            StdCommand::new("git")
                .args(args)
                .current_dir(&self.dir)
                .status()
                .unwrap()
                .success()
        );
    }

    fn commit_initial_file(&self) {
        self.write("committed.txt", "initial\n");
        self.git(&["add", "committed.txt"]);
        self.git(&["commit", "-q", "-m", "initial"]);
    }

    fn autoship(&self) -> Command {
        let mut cmd = Command::cargo_bin("autoship").unwrap();
        cmd.current_dir(&self.dir);
        cmd
    }
}

impl Drop for TestRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn bare_remote() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "autoship-cli-test-remote-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    assert!(
        StdCommand::new("git")
            .args(["init", "-q", "--bare"])
            .current_dir(&dir)
            .status()
            .unwrap()
            .success()
    );
    dir
}

#[test]
fn no_staged_changes_reports_nothing_to_ship() {
    let repo = TestRepo::new();
    repo.commit_initial_file();

    let output = repo.autoship().output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No staged changes found."));
}

#[test]
fn dry_run_shows_the_plan_without_mutating_the_repository() {
    let repo = TestRepo::new();
    repo.commit_initial_file();
    repo.write("new.txt", "content\n");
    repo.git(&["add", "new.txt"]);

    let output = repo.autoship().arg("--dry-run").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Autoship Plan"));
    assert!(stdout.contains("Branch:"));
    assert!(stdout.contains("Commit:"));
    assert!(stdout.contains("No changes were made."));

    // Nothing should have been committed, and the staged file should still be staged.
    let log = StdCommand::new("git")
        .args(["log", "--oneline"])
        .current_dir(&repo.dir)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&log.stdout).lines().count(), 1);
    let staged = StdCommand::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(&repo.dir)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&staged.stdout).trim(), "new.txt");
}

#[test]
fn full_workflow_creates_a_branch_commits_and_pushes_with_upstream() {
    let repo = TestRepo::new();
    repo.commit_initial_file();
    repo.write("new.txt", "content\n");
    repo.git(&["add", "new.txt"]);
    let remote = bare_remote();
    repo.git(&["remote", "add", "origin", remote.to_str().unwrap()]);

    // Accept commit message, choose the suggested (new) branch, create it, commit, use the
    // single remote, and push.
    let output = repo
        .autoship()
        .write_stdin("y\n2\ny\ny\ny\ny\n")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "stdout was:\n{stdout}");
    assert!(stdout.contains("✓ Branch created:"));
    assert!(stdout.contains("✓ Changes committed"));
    assert!(stdout.contains("✓ Pushed origin/"));
    assert!(stdout.contains("✓ Upstream configured"));

    fs::remove_dir_all(&remote).unwrap();
}

#[test]
fn missing_remote_is_reported_clearly() {
    let repo = TestRepo::new();
    repo.commit_initial_file();
    repo.write("new.txt", "content\n");
    repo.git(&["add", "new.txt"]);

    // Accept commit message, stay on the current branch, commit.
    let output = repo.autoship().write_stdin("y\n1\ny\n").output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "stdout was:\n{stdout}");
    assert!(stdout.contains("No Git remote configured."));
}

#[test]
fn multiple_remotes_prompt_with_a_numbered_list() {
    let repo = TestRepo::new();
    repo.commit_initial_file();
    repo.write("new.txt", "content\n");
    repo.git(&["add", "new.txt"]);
    repo.git(&["remote", "add", "origin", "git@example.com:user/repo.git"]);
    repo.git(&[
        "remote",
        "add",
        "upstream",
        "git@example.com:company/repo.git",
    ]);

    // Accept commit message, stay on the current branch, commit, pick remote 1, decline push.
    let output = repo
        .autoship()
        .write_stdin("y\n1\ny\n1\nn\n")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "stdout was:\n{stdout}");
    assert!(stdout.contains("? Push to:"));
    assert!(stdout.contains("1. origin"));
    assert!(stdout.contains("2. upstream"));
    assert!(stdout.contains("3. Custom remote"));
    assert!(stdout.contains("Push skipped."));
}

#[test]
fn invalid_version_file_is_reported_without_crashing() {
    let repo = TestRepo::new();
    repo.commit_initial_file();
    repo.write("VERSION", "not-a-version\n");
    repo.write("new.txt", "content\n");
    repo.git(&["add", "VERSION", "new.txt"]);

    // Accept commit message, stay on the current branch, decline commit.
    let output = repo.autoship().write_stdin("y\n1\nn\n").output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "stdout was:\n{stdout}");
    assert!(stdout.contains("Current version: not-a-version"));
    assert!(stdout.contains("Suggested version: none (unrecognized version format, skipping)"));
}

#[test]
fn push_failure_is_reported_as_an_error() {
    let repo = TestRepo::new();
    repo.commit_initial_file();
    repo.write("new.txt", "content\n");
    repo.git(&["add", "new.txt"]);
    let missing_remote_path = repo.dir.join("does-not-exist");
    repo.git(&[
        "remote",
        "add",
        "origin",
        missing_remote_path.to_str().unwrap(),
    ]);

    // Accept commit message, stay on the current branch, commit, use origin, push.
    let output = repo
        .autoship()
        .write_stdin("y\n1\ny\ny\ny\n")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Error:"));
}

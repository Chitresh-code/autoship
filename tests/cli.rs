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
    assert!(stdout.contains("Branch"));
    assert!(stdout.contains("Commit"));
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

    // --yes accepts the suggested branch, commits, and pushes to the single remote.
    let output = repo.autoship().arg("--yes").output().unwrap();

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

    let output = repo.autoship().arg("--yes").output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "stdout was:\n{stdout}");
    assert!(stdout.contains("No Git remote configured."));
}

#[test]
fn multiple_remotes_default_to_the_configured_preference_without_prompting() {
    let repo = TestRepo::new();
    repo.commit_initial_file();
    repo.write("new.txt", "content\n");
    repo.git(&["add", "new.txt"]);
    let origin_remote = bare_remote();
    let upstream_remote = bare_remote();
    // Added out of order to prove `origin` is chosen deliberately, not just first-listed.
    repo.git(&[
        "remote",
        "add",
        "upstream",
        upstream_remote.to_str().unwrap(),
    ]);
    repo.git(&["remote", "add", "origin", origin_remote.to_str().unwrap()]);

    let output = repo.autoship().arg("--yes").output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "stdout was:\n{stdout}");
    assert!(stdout.contains("Remote selected: origin"));
    assert!(stdout.contains("✓ Pushed origin/"));

    fs::remove_dir_all(&origin_remote).unwrap();
    fs::remove_dir_all(&upstream_remote).unwrap();
}

#[test]
fn accepting_a_version_bump_writes_it_to_the_project_file() {
    let repo = TestRepo::new();
    repo.commit_initial_file();
    repo.write("VERSION", "1.0.0\n");
    repo.git(&["add", "VERSION"]);
    repo.git(&["commit", "-q", "-m", "add version file"]);
    repo.write("new.txt", "content\n");
    repo.git(&["add", "new.txt"]);

    let output = repo.autoship().arg("--yes").output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "stdout was:\n{stdout}");
    assert!(stdout.contains("Suggested version: 1.1.0"));
    assert!(stdout.contains("✓ Version updated to 1.1.0"));

    let contents = fs::read_to_string(repo.dir.join("VERSION")).unwrap();
    assert_eq!(contents.trim(), "1.1.0");
}

#[test]
fn invalid_version_file_is_reported_without_crashing() {
    let repo = TestRepo::new();
    repo.commit_initial_file();
    repo.write("VERSION", "not-a-version\n");
    repo.write("new.txt", "content\n");
    repo.git(&["add", "VERSION", "new.txt"]);

    let output = repo.autoship().arg("--yes").output().unwrap();

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

    let output = repo.autoship().arg("--yes").output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Error:"));
}

#[test]
fn without_yes_or_a_terminal_fails_clearly_instead_of_guessing() {
    let repo = TestRepo::new();
    repo.commit_initial_file();
    repo.write("new.txt", "content\n");
    repo.git(&["add", "new.txt"]);

    // No --yes, and assert_cmd never attaches a real terminal: the first prompt (the commit
    // message) must fail clearly rather than silently defaulting or hanging on stdin.
    let output = repo.autoship().output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--yes"));
}

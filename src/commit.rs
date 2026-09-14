use std::fmt;
use std::path::Path;

use crate::bump::{self, ChangeKind, Classification};
use crate::git::{FileStatus, StagedFile};

/// A generated Conventional Commit message: `type(scope): subject`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitMessage {
    pub commit_type: &'static str,
    pub scope: Option<String>,
    pub subject: String,
}

impl fmt::Display for CommitMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.scope {
            Some(scope) => write!(f, "{}({}): {}", self.commit_type, scope, self.subject),
            None => write!(f, "{}: {}", self.commit_type, self.subject),
        }
    }
}

fn infer_type(files: &[StagedFile], classification: &Classification) -> &'static str {
    match classification.kind {
        ChangeKind::Docs => "docs",
        ChangeKind::Test => "test",
        ChangeKind::Chore => {
            if files.iter().any(|f| f.path.starts_with(".github/")) {
                "ci"
            } else if files.iter().any(|f| bump::is_lockfile(&f.path)) {
                "build"
            } else {
                "chore"
            }
        }
        // ponytail: can't distinguish a bug fix from an internal refactor without semantic
        // understanding, so patch-level code changes default to `fix`; the confirmation
        // prompt lets the user correct it (`edit`).
        ChangeKind::Code => match classification.bump {
            Some(bump::Bump::Minor) => "feat",
            _ => "fix",
        },
    }
}

/// If every changed file sits under the same immediate subdirectory (skipping common
/// roots like `src/` or `tests/`), use that as the Conventional Commit scope.
fn infer_scope(files: &[StagedFile]) -> Option<String> {
    fn component(path: &str) -> Option<&str> {
        let parts: Vec<&str> = path.split('/').collect();
        let skip_root = matches!(parts.first(), Some(&"src" | &"lib" | &"tests" | &"test"));
        let idx = usize::from(skip_root);
        (parts.len() > idx + 1).then(|| parts[idx])
    }

    let mut components = files.iter().map(|f| component(&f.path));
    let first = components.next()??;
    components
        .all(|c| c == Some(first))
        .then(|| first.to_string())
}

fn infer_subject(files: &[StagedFile], kind: ChangeKind) -> String {
    if let [only] = files {
        let stem = Path::new(&only.path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&only.path);
        let verb = match only.status {
            FileStatus::Added => "add",
            FileStatus::Deleted => "remove",
            FileStatus::Renamed => "rename",
            FileStatus::Modified | FileStatus::Other => "update",
        };
        return format!("{verb} {stem}");
    }

    match kind {
        ChangeKind::Docs => "update documentation".to_string(),
        ChangeKind::Test => "update tests".to_string(),
        ChangeKind::Chore | ChangeKind::Code => format!("update {} files", files.len()),
    }
}

/// Suggests a Conventional Commit message for a staged diff, without AI.
pub fn suggest(files: &[StagedFile], classification: &Classification) -> CommitMessage {
    CommitMessage {
        commit_type: infer_type(files, classification),
        scope: infer_scope(files),
        subject: infer_subject(files, classification.kind),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bump;

    fn file(path: &str, status: FileStatus) -> StagedFile {
        StagedFile {
            path: path.to_string(),
            status,
        }
    }

    #[test]
    fn suggests_feat_for_a_new_file_with_scope() {
        let files = [file("src/auth/login.rs", FileStatus::Added)];
        let classification = bump::classify(&files);

        let commit = suggest(&files, &classification);

        assert_eq!(commit.to_string(), "feat(auth): add login");
    }

    #[test]
    fn suggests_fix_for_a_modified_file() {
        let files = [file("src/auth/login.rs", FileStatus::Modified)];
        let classification = bump::classify(&files);

        let commit = suggest(&files, &classification);

        assert_eq!(commit.to_string(), "fix(auth): update login");
    }

    #[test]
    fn suggests_docs_type_with_no_scope_for_mixed_doc_dirs() {
        let files = [
            file("docs/architecture.md", FileStatus::Modified),
            file("README.md", FileStatus::Modified),
        ];
        let classification = bump::classify(&files);

        let commit = suggest(&files, &classification);

        assert_eq!(commit.commit_type, "docs");
        assert_eq!(commit.scope, None);
        assert_eq!(commit.subject, "update documentation");
    }

    #[test]
    fn suggests_ci_type_for_github_workflow_files() {
        let files = [file(".github/workflows/ci.yml", FileStatus::Modified)];
        let classification = bump::classify(&files);

        let commit = suggest(&files, &classification);

        assert_eq!(commit.commit_type, "ci");
    }

    #[test]
    fn suggests_build_type_for_lockfile_changes() {
        let files = [file("Cargo.lock", FileStatus::Modified)];
        let classification = bump::classify(&files);

        let commit = suggest(&files, &classification);

        assert_eq!(commit.commit_type, "build");
    }

    #[test]
    fn no_scope_when_files_span_different_directories() {
        let files = [
            file("src/auth/login.rs", FileStatus::Modified),
            file("src/config/loader.rs", FileStatus::Modified),
        ];

        let scope = infer_scope(&files);

        assert_eq!(scope, None);
    }
}

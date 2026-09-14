use semver::Version;

use crate::git::{FileStatus, StagedFile};

/// A SemVer bump size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bump {
    // ponytail: never suggested by `classify` (breaking-change detection needs semantic
    // understanding this heuristic doesn't have); reserved for the manual `ship --version
    // major` override, added when the CLI flag is wired up.
    #[allow(dead_code)]
    Major,
    Minor,
    Patch,
}

impl Bump {
    /// The label shown to the user, matching the CLI's `ship --version <label>` flag.
    pub fn label(self) -> &'static str {
        match self {
            Bump::Major => "major",
            Bump::Minor => "minor",
            Bump::Patch => "patch",
        }
    }

    /// Applies this bump to `current`, following standard SemVer reset rules.
    pub fn apply(self, current: &Version) -> Version {
        match self {
            Bump::Major => Version::new(current.major + 1, 0, 0),
            Bump::Minor => Version::new(current.major, current.minor + 1, 0),
            Bump::Patch => Version::new(current.major, current.minor, current.patch + 1),
        }
    }
}

/// What kind of staged change was detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Docs,
    Test,
    Chore,
    Code,
}

impl ChangeKind {
    pub fn label(self) -> &'static str {
        match self {
            ChangeKind::Docs => "docs",
            ChangeKind::Test => "test",
            ChangeKind::Chore => "chore",
            ChangeKind::Code => "code",
        }
    }
}

/// The result of classifying a staged diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Classification {
    pub kind: ChangeKind,
    /// `None` means no version bump is suggested (docs/tests/chore-only changes).
    pub bump: Option<Bump>,
}

fn is_docs(path: &str) -> bool {
    path.starts_with("docs/") || path.ends_with(".md")
}

fn is_test(path: &str) -> bool {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    path.starts_with("tests/")
        || path.contains("/tests/")
        || path.contains("/test/")
        || file_name.starts_with("test_")
        || file_name.contains(".test.")
        || file_name.contains("_test.")
}

/// True for dependency lockfiles, shared with commit-type inference (`build` vs `chore`).
pub(crate) fn is_lockfile(path: &str) -> bool {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    matches!(
        file_name,
        "Cargo.lock" | "package-lock.json" | "yarn.lock" | "pnpm-lock.yaml" | "poetry.lock"
    )
}

fn is_chore(path: &str) -> bool {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    path.starts_with(".github/") || file_name.starts_with('.') || is_lockfile(path)
}

/// Classifies a staged diff without AI, using file paths and their change status.
///
/// This is deliberately conservative: it never suggests a major bump, since detecting
/// breaking changes requires semantic understanding this heuristic doesn't have. Major
/// bumps remain a manual choice (`ship --version major`).
pub fn classify(files: &[StagedFile]) -> Classification {
    if !files.is_empty() && files.iter().all(|f| is_docs(&f.path)) {
        return Classification {
            kind: ChangeKind::Docs,
            bump: None,
        };
    }
    if !files.is_empty() && files.iter().all(|f| is_test(&f.path)) {
        return Classification {
            kind: ChangeKind::Test,
            bump: None,
        };
    }
    if !files.is_empty() && files.iter().all(|f| is_chore(&f.path)) {
        return Classification {
            kind: ChangeKind::Chore,
            bump: None,
        };
    }

    let has_new_file = files
        .iter()
        .any(|f| matches!(f.status, FileStatus::Added | FileStatus::Renamed));
    Classification {
        kind: ChangeKind::Code,
        bump: Some(if has_new_file {
            Bump::Minor
        } else {
            Bump::Patch
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, status: FileStatus) -> StagedFile {
        StagedFile {
            path: path.to_string(),
            status,
        }
    }

    #[test]
    fn bump_apply_resets_lower_components() {
        let v = Version::new(1, 8, 2);
        assert_eq!(Bump::Major.apply(&v), Version::new(2, 0, 0));
        assert_eq!(Bump::Minor.apply(&v), Version::new(1, 9, 0));
        assert_eq!(Bump::Patch.apply(&v), Version::new(1, 8, 3));
    }

    #[test]
    fn classifies_docs_only_change_with_no_bump() {
        let files = [
            file("docs/architecture.md", FileStatus::Modified),
            file("README.md", FileStatus::Modified),
        ];

        let result = classify(&files);

        assert_eq!(result.kind, ChangeKind::Docs);
        assert_eq!(result.bump, None);
    }

    #[test]
    fn classifies_test_only_change_with_no_bump() {
        let files = [file("tests/auth_test.rs", FileStatus::Modified)];

        let result = classify(&files);

        assert_eq!(result.kind, ChangeKind::Test);
        assert_eq!(result.bump, None);
    }

    #[test]
    fn classifies_chore_only_change_with_no_bump() {
        let files = [
            file("Cargo.lock", FileStatus::Modified),
            file(".github/workflows/ci.yml", FileStatus::Modified),
        ];

        let result = classify(&files);

        assert_eq!(result.kind, ChangeKind::Chore);
        assert_eq!(result.bump, None);
    }

    #[test]
    fn classifies_new_file_as_minor() {
        let files = [file("src/auth.rs", FileStatus::Added)];

        let result = classify(&files);

        assert_eq!(result.kind, ChangeKind::Code);
        assert_eq!(result.bump, Some(Bump::Minor));
    }

    #[test]
    fn classifies_modified_existing_file_as_patch() {
        let files = [file("src/auth.rs", FileStatus::Modified)];

        let result = classify(&files);

        assert_eq!(result.kind, ChangeKind::Code);
        assert_eq!(result.bump, Some(Bump::Patch));
    }

    #[test]
    fn mixed_code_and_docs_is_classified_as_code() {
        let files = [
            file("src/auth.rs", FileStatus::Modified),
            file("README.md", FileStatus::Modified),
        ];

        let result = classify(&files);

        assert_eq!(result.kind, ChangeKind::Code);
    }

    #[test]
    fn never_suggests_a_major_bump() {
        let files = [file("src/lib.rs", FileStatus::Deleted)];

        let result = classify(&files);

        assert_ne!(result.bump, Some(Bump::Major));
    }
}

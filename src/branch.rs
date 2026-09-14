use crate::commit::CommitMessage;

/// Keep branch names short and readable (CLAUDE.md: "avoid unnecessarily long branch names").
const MAX_SLUG_LEN: usize = 50;

/// Default branch-name prefix for a Conventional Commit type (PRD section 11).
///
/// ponytail: hardcoded here; becomes configurable via `[branch.prefixes]` in `.ship.toml`
/// once configuration support lands (PRD section 39, step 15).
fn prefix(commit_type: &str) -> &'static str {
    match commit_type {
        "feat" => "feature/",
        "fix" => "fix/",
        "docs" => "docs/",
        "refactor" => "refactor/",
        _ => "chore/",
    }
}

/// Lowercase, hyphen-separated slug built from whole words, never cut mid-word.
fn slugify(subject: &str) -> String {
    let words = subject
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase);

    let mut slug = String::new();
    for word in words {
        let separator_len = usize::from(!slug.is_empty());
        if !slug.is_empty() && slug.len() + separator_len + word.len() > MAX_SLUG_LEN {
            break;
        }
        if !slug.is_empty() {
            slug.push('-');
        }
        slug.push_str(&word);
    }
    slug
}

/// Suggests a branch name for a Conventional Commit message, without AI.
pub fn suggest(commit: &CommitMessage) -> String {
    format!("{}{}", prefix(commit.commit_type), slugify(&commit.subject))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(commit_type: &'static str, subject: &str) -> CommitMessage {
        CommitMessage {
            commit_type,
            scope: None,
            subject: subject.to_string(),
        }
    }

    #[test]
    fn feat_gets_the_feature_prefix() {
        assert_eq!(suggest(&commit("feat", "add login")), "feature/add-login");
    }

    #[test]
    fn fix_gets_the_fix_prefix() {
        assert_eq!(suggest(&commit("fix", "update login")), "fix/update-login");
    }

    #[test]
    fn docs_gets_the_docs_prefix() {
        assert_eq!(
            suggest(&commit("docs", "update documentation")),
            "docs/update-documentation"
        );
    }

    #[test]
    fn refactor_gets_the_refactor_prefix() {
        assert_eq!(
            suggest(&commit("refactor", "simplify request handling")),
            "refactor/simplify-request-handling"
        );
    }

    #[test]
    fn other_types_default_to_the_chore_prefix() {
        for commit_type in ["test", "build", "ci", "chore", "perf", "revert"] {
            assert_eq!(
                suggest(&commit(commit_type, "update stuff")),
                "chore/update-stuff"
            );
        }
    }

    #[test]
    fn slug_drops_punctuation_and_collapses_separators() {
        assert_eq!(
            slugify("Handle missing ENV vars, again!"),
            "handle-missing-env-vars-again"
        );
    }

    #[test]
    fn slug_truncates_on_a_whole_word_boundary() {
        let subject = "update one two three four five six seven eight nine ten eleven twelve";
        let words: Vec<&str> = subject.split(' ').collect();

        let slug = slugify(subject);

        assert!(slug.len() <= MAX_SLUG_LEN);
        assert!(!slug.ends_with('-'));
        assert_eq!(slug, words[..slug.split('-').count()].join("-"));
    }
}

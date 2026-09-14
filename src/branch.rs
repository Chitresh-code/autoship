use std::collections::HashMap;

use crate::commit::CommitMessage;

/// Keep branch names short and readable (CLAUDE.md: "avoid unnecessarily long branch names").
const MAX_SLUG_LEN: usize = 50;

/// Default branch-name prefix for a Conventional Commit type (PRD section 11), overridable
/// via `[branch.prefixes]` in `.ship.toml`.
fn prefix<'a>(commit_type: &str, overrides: &'a HashMap<String, String>) -> &'a str {
    if let Some(custom) = overrides.get(commit_type) {
        return custom;
    }
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
pub fn suggest(commit: &CommitMessage, prefix_overrides: &HashMap<String, String>) -> String {
    format!(
        "{}{}",
        prefix(commit.commit_type, prefix_overrides),
        slugify(&commit.subject)
    )
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

    fn no_overrides() -> HashMap<String, String> {
        HashMap::new()
    }

    #[test]
    fn feat_gets_the_feature_prefix() {
        assert_eq!(
            suggest(&commit("feat", "add login"), &no_overrides()),
            "feature/add-login"
        );
    }

    #[test]
    fn fix_gets_the_fix_prefix() {
        assert_eq!(
            suggest(&commit("fix", "update login"), &no_overrides()),
            "fix/update-login"
        );
    }

    #[test]
    fn docs_gets_the_docs_prefix() {
        assert_eq!(
            suggest(&commit("docs", "update documentation"), &no_overrides()),
            "docs/update-documentation"
        );
    }

    #[test]
    fn refactor_gets_the_refactor_prefix() {
        assert_eq!(
            suggest(
                &commit("refactor", "simplify request handling"),
                &no_overrides()
            ),
            "refactor/simplify-request-handling"
        );
    }

    #[test]
    fn other_types_default_to_the_chore_prefix() {
        for commit_type in ["test", "build", "ci", "chore", "perf", "revert"] {
            assert_eq!(
                suggest(&commit(commit_type, "update stuff"), &no_overrides()),
                "chore/update-stuff"
            );
        }
    }

    #[test]
    fn a_configured_prefix_overrides_the_default() {
        let mut overrides = HashMap::new();
        overrides.insert("feat".to_string(), "f/".to_string());

        assert_eq!(
            suggest(&commit("feat", "add login"), &overrides),
            "f/add-login"
        );
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

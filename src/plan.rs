use std::fmt::Write as _;

/// Renders the `--dry-run` plan summary (PRD section 15).
///
/// `version` and `remote` are omitted from the plan when there is nothing to report (no
/// detected version, no version bump, or no configured remote).
pub fn render(
    version: Option<(&str, &str)>,
    branch: (&str, &str),
    commit: &str,
    remote: Option<&str>,
) -> String {
    let mut out = String::from("Autoship Plan\n");
    let (current_branch, suggested_branch) = branch;

    if let Some((current, suggested)) = version {
        let _ = write!(out, "\nVersion:\n  {current} \u{2192} {suggested}\n");
    }

    let _ = write!(
        out,
        "\nBranch:\n  {current_branch} \u{2192} {suggested_branch}\n"
    );
    let _ = write!(out, "\nCommit:\n  {commit}\n");

    if let Some(remote) = remote {
        let _ = write!(out, "\nRemote:\n  {remote}\n");
        let _ = write!(out, "\nPush:\n  {remote}/{suggested_branch}\n");
    }

    out.push_str("\nNo changes were made.\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_the_full_plan_in_prd_order() {
        let text = render(
            Some(("1.8.2", "1.9.0")),
            ("main", "feature/oauth-login"),
            "feat(auth): add OAuth login",
            Some("origin"),
        );

        assert_eq!(
            text,
            "Autoship Plan\n\
             \n\
             Version:\n  1.8.2 \u{2192} 1.9.0\n\
             \n\
             Branch:\n  main \u{2192} feature/oauth-login\n\
             \n\
             Commit:\n  feat(auth): add OAuth login\n\
             \n\
             Remote:\n  origin\n\
             \n\
             Push:\n  origin/feature/oauth-login\n\
             \n\
             No changes were made.\n"
        );
    }

    #[test]
    fn omits_version_and_remote_sections_when_absent() {
        let text = render(None, ("main", "fix/thing"), "fix: thing", None);

        assert_eq!(
            text,
            "Autoship Plan\n\
             \n\
             Branch:\n  main \u{2192} fix/thing\n\
             \n\
             Commit:\n  fix: thing\n\
             \n\
             No changes were made.\n"
        );
    }
}

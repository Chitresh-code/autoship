use comfy_table::Table;
use comfy_table::presets::UTF8_BORDERS_ONLY;

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
    let (current_branch, suggested_branch) = branch;

    let mut table = Table::new();
    table.load_style(UTF8_BORDERS_ONLY);
    table.set_header(vec!["Field", "Value"]);

    if let Some((current, suggested)) = version {
        table.add_row(vec![
            "Version".to_string(),
            format!("{current} \u{2192} {suggested}"),
        ]);
    }
    table.add_row(vec![
        "Branch".to_string(),
        format!("{current_branch} \u{2192} {suggested_branch}"),
    ]);
    table.add_row(vec!["Commit".to_string(), commit.to_string()]);
    if let Some(remote) = remote {
        table.add_row(vec!["Remote".to_string(), remote.to_string()]);
        table.add_row(vec![
            "Push".to_string(),
            format!("{remote}/{suggested_branch}"),
        ]);
    }

    format!("Autoship Plan\n\n{table}\n\nNo changes were made.\n")
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

        assert!(text.starts_with("Autoship Plan\n"));
        assert!(text.contains("Version"));
        assert!(text.contains("1.8.2 \u{2192} 1.9.0"));
        assert!(text.contains("Branch"));
        assert!(text.contains("main \u{2192} feature/oauth-login"));
        assert!(text.contains("Commit"));
        assert!(text.contains("feat(auth): add OAuth login"));
        assert!(text.contains("Remote"));
        assert!(text.contains("origin"));
        assert!(text.contains("Push"));
        assert!(text.contains("origin/feature/oauth-login"));
        assert!(text.ends_with("No changes were made.\n"));
    }

    #[test]
    fn omits_version_and_remote_sections_when_absent() {
        let text = render(None, ("main", "fix/thing"), "fix: thing", None);

        assert!(!text.contains("Version"));
        assert!(text.contains("Branch"));
        assert!(text.contains("main \u{2192} fix/thing"));
        assert!(text.contains("Commit"));
        assert!(text.contains("fix: thing"));
        assert!(!text.contains("Remote"));
        assert!(!text.contains("Push"));
        assert!(text.ends_with("No changes were made.\n"));
    }
}

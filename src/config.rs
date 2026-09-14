use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Resolved Autoship configuration, merged from global and project `.autoship.toml` files.
///
/// Only options with a concrete use case exist here (CLAUDE.md: "avoid speculative
/// configuration"): branch prefixes (`branch::prefix`) and a preferred remote
/// (`default_remote` in `main.rs`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    /// Overrides for `branch::prefix`, keyed by Conventional Commit type (e.g. "feat").
    pub branch_prefixes: HashMap<String, String>,
    /// Preferred remote name, used instead of the "origin"-first default.
    pub remote: Option<String>,
}

fn parse(contents: &str) -> Result<Config> {
    let value: toml::Value = toml::from_str(contents).context("invalid .autoship.toml")?;
    let branch_prefixes = value
        .get("branch")
        .and_then(|b| b.get("prefixes"))
        .and_then(|p| p.as_table())
        .map(|table| {
            table
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();
    let remote = value
        .get("git")
        .and_then(|g| g.get("remote"))
        .and_then(|r| r.as_str())
        .map(str::to_string);
    Ok(Config {
        branch_prefixes,
        remote,
    })
}

fn read(path: &Path) -> Result<Option<Config>> {
    if !path.is_file() {
        return Ok(None);
    }
    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    parse(&contents).map(Some)
}

/// Applies project configuration on top of global configuration, field by field.
fn merge(global: Config, project: Option<Config>) -> Config {
    let mut merged = global;
    if let Some(project) = project {
        if !project.branch_prefixes.is_empty() {
            merged.branch_prefixes = project.branch_prefixes;
        }
        if project.remote.is_some() {
            merged.remote = project.remote;
        }
    }
    merged
}

/// The platform-appropriate user configuration directory for Autoship's global config file.
fn global_config_path() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        env::var_os("HOME").map(|home| {
            PathBuf::from(home).join("Library/Application Support/autoship/config.toml")
        })
    } else if cfg!(target_os = "windows") {
        env::var_os("APPDATA").map(|dir| PathBuf::from(dir).join("autoship/config.toml"))
    } else {
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .map(|dir| dir.join("autoship/config.toml"))
    }
}

/// Loads and merges global and project (`<repo_root>/.autoship.toml`) configuration.
pub fn load(repo_root: &Path) -> Result<Config> {
    let global = match global_config_path() {
        Some(path) => read(&path)?,
        None => None,
    };
    let project = read(&repo_root.join(".autoship.toml"))?;
    Ok(merge(global.unwrap_or_default(), project))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_branch_prefixes_and_remote() {
        let config =
            parse("[branch.prefixes]\nfeat = \"f/\"\n\n[git]\nremote = \"upstream\"\n").unwrap();

        assert_eq!(config.branch_prefixes.get("feat"), Some(&"f/".to_string()));
        assert_eq!(config.remote, Some("upstream".to_string()));
    }

    #[test]
    fn missing_sections_produce_empty_defaults() {
        let config = parse("").unwrap();

        assert!(config.branch_prefixes.is_empty());
        assert_eq!(config.remote, None);
    }

    #[test]
    fn project_branch_prefixes_override_global_wholesale() {
        let mut global = Config::default();
        global
            .branch_prefixes
            .insert("feat".to_string(), "feature/".to_string());
        global.remote = Some("origin".to_string());

        let mut project = Config::default();
        project
            .branch_prefixes
            .insert("fix".to_string(), "bugfix/".to_string());

        let merged = merge(global, Some(project));

        assert_eq!(
            merged.branch_prefixes.get("fix"),
            Some(&"bugfix/".to_string())
        );
        assert!(!merged.branch_prefixes.contains_key("feat"));
        assert_eq!(merged.remote, Some("origin".to_string()));
    }

    #[test]
    fn load_reads_project_autoship_toml_from_the_repo_root() {
        let dir = std::env::temp_dir().join(format!(
            "autoship-config-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(".autoship.toml"),
            "[branch.prefixes]\nfeat = \"f/\"\n\n[git]\nremote = \"upstream\"\n",
        )
        .unwrap();

        // Project values always win, regardless of whatever global config (if any) exists on
        // the machine running this test.
        let config = load(&dir).unwrap();

        assert_eq!(config.branch_prefixes.get("feat"), Some(&"f/".to_string()));
        assert_eq!(config.remote, Some("upstream".to_string()));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn absent_project_config_keeps_global() {
        let global = Config {
            remote: Some("origin".to_string()),
            ..Config::default()
        };

        let merged = merge(global.clone(), None);

        assert_eq!(merged, global);
    }
}

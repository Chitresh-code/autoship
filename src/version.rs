use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// A version found by a [`VersionProvider`] in a project file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedVersion {
    pub ecosystem: &'static str,
    pub file: PathBuf,
    pub version: String,
}

trait VersionProvider {
    fn file_name(&self) -> &'static str;
    fn ecosystem(&self) -> &'static str;
    fn extract(&self, contents: &str) -> Result<Option<String>>;
}

struct NodeProvider;

impl VersionProvider for NodeProvider {
    fn file_name(&self) -> &'static str {
        "package.json"
    }

    fn ecosystem(&self) -> &'static str {
        "Node.js"
    }

    fn extract(&self, contents: &str) -> Result<Option<String>> {
        let value: serde_json::Value =
            serde_json::from_str(contents).context("invalid package.json")?;
        Ok(value
            .get("version")
            .and_then(|v| v.as_str())
            .map(str::to_string))
    }
}

struct CargoProvider;

impl VersionProvider for CargoProvider {
    fn file_name(&self) -> &'static str {
        "Cargo.toml"
    }

    fn ecosystem(&self) -> &'static str {
        "Rust"
    }

    fn extract(&self, contents: &str) -> Result<Option<String>> {
        let value: toml::Value = toml::from_str(contents).context("invalid Cargo.toml")?;
        Ok(value
            .get("package")
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str())
            .map(str::to_string))
    }
}

struct PyProjectProvider;

impl VersionProvider for PyProjectProvider {
    fn file_name(&self) -> &'static str {
        "pyproject.toml"
    }

    fn ecosystem(&self) -> &'static str {
        "Python"
    }

    fn extract(&self, contents: &str) -> Result<Option<String>> {
        let value: toml::Value = toml::from_str(contents).context("invalid pyproject.toml")?;
        let project_version = value
            .get("project")
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str());
        let poetry_version = value
            .get("tool")
            .and_then(|t| t.get("poetry"))
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str());
        Ok(project_version.or(poetry_version).map(str::to_string))
    }
}

struct PlainFileProvider {
    file: &'static str,
}

impl VersionProvider for PlainFileProvider {
    fn file_name(&self) -> &'static str {
        self.file
    }

    fn ecosystem(&self) -> &'static str {
        "Generic"
    }

    fn extract(&self, contents: &str) -> Result<Option<String>> {
        let trimmed = contents.trim();
        Ok(if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        })
    }
}

fn providers() -> Vec<Box<dyn VersionProvider>> {
    vec![
        Box::new(NodeProvider),
        Box::new(CargoProvider),
        Box::new(PyProjectProvider),
        Box::new(PlainFileProvider { file: "VERSION" }),
        Box::new(PlainFileProvider {
            file: "version.txt",
        }),
    ]
}

/// Detects the project version by checking known ecosystem files, in priority order.
pub fn detect(repo_root: &Path) -> Result<Option<DetectedVersion>> {
    for provider in providers() {
        let path = repo_root.join(provider.file_name());
        if !path.is_file() {
            continue;
        }
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if let Some(version) = provider.extract(&contents)? {
            return Ok(Some(DetectedVersion {
                ecosystem: provider.ecosystem(),
                file: path,
                version,
            }));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("autoship-version-test-{nanos}-{n}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn detects_node_version() {
        let dir = temp_dir();
        fs::write(
            dir.join("package.json"),
            r#"{"name": "x", "version": "1.2.3"}"#,
        )
        .unwrap();

        let detected = detect(&dir).unwrap().unwrap();

        assert_eq!(detected.ecosystem, "Node.js");
        assert_eq!(detected.version, "1.2.3");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn detects_cargo_version() {
        let dir = temp_dir();
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"x\"\nversion = \"0.4.0\"\n",
        )
        .unwrap();

        let detected = detect(&dir).unwrap().unwrap();

        assert_eq!(detected.ecosystem, "Rust");
        assert_eq!(detected.version, "0.4.0");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn detects_pyproject_version() {
        let dir = temp_dir();
        fs::write(
            dir.join("pyproject.toml"),
            "[project]\nname = \"x\"\nversion = \"2.0.0\"\n",
        )
        .unwrap();

        let detected = detect(&dir).unwrap().unwrap();

        assert_eq!(detected.ecosystem, "Python");
        assert_eq!(detected.version, "2.0.0");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn detects_generic_version_file() {
        let dir = temp_dir();
        fs::write(dir.join("VERSION"), "5.6.7\n").unwrap();

        let detected = detect(&dir).unwrap().unwrap();

        assert_eq!(detected.ecosystem, "Generic");
        assert_eq!(detected.version, "5.6.7");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn returns_none_when_no_version_file_present() {
        let dir = temp_dir();

        let detected = detect(&dir).unwrap();

        assert!(detected.is_none());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn prefers_node_over_rust_when_both_present() {
        let dir = temp_dir();
        fs::write(dir.join("package.json"), r#"{"version": "1.0.0"}"#).unwrap();
        fs::write(dir.join("Cargo.toml"), "[package]\nversion = \"2.0.0\"\n").unwrap();

        let detected = detect(&dir).unwrap().unwrap();

        assert_eq!(detected.ecosystem, "Node.js");
        fs::remove_dir_all(&dir).unwrap();
    }
}

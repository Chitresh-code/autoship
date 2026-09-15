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

    /// Returns `contents` with `current` replaced by `new_version`, preserving everything
    /// else in the file (formatting, comments, key order) exactly as-is.
    fn write(&self, contents: &str, current: &str, new_version: &str) -> Result<String>;
}

/// Replaces `key = "old"` inside a specific top-level TOML table (dotted for nested tables,
/// e.g. `tool.poetry`), leaving every other byte of the file untouched. Returns `None` if the
/// table or key isn't found.
fn replace_toml_string_in_table(
    contents: &str,
    table: &str,
    key: &str,
    new_value: &str,
) -> Option<String> {
    let mut out = String::with_capacity(contents.len() + 8);
    let mut in_table = false;
    let mut replaced = false;
    for line in contents.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let trimmed = body.trim();
        if !replaced {
            if let Some(name) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                in_table = name.trim() == table;
                out.push_str(line);
                continue;
            }
            if in_table
                && let Some(eq) = trimmed.find('=')
                && trimmed[..eq].trim() == key
            {
                let indent = &body[..body.len() - body.trim_start().len()];
                let ending = &line[body.len()..];
                out.push_str(indent);
                out.push_str(key);
                out.push_str(" = \"");
                out.push_str(new_value);
                out.push('"');
                out.push_str(ending);
                replaced = true;
                continue;
            }
        }
        out.push_str(line);
    }
    replaced.then_some(out)
}

/// Replaces a `"key": "old"` pair that sits directly in the JSON document's top-level object
/// (not nested inside an array or another object, e.g. a dependency entry), leaving every
/// other byte untouched. Returns `None` if no such top-level key is found.
fn replace_json_top_level_string(contents: &str, key: &str, new_value: &str) -> Option<String> {
    let quoted_key = format!("\"{key}\"");
    let bytes = contents.as_bytes();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if in_string {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match c {
            '"' if depth == 1 && contents[i..].starts_with(&quoted_key) => {
                let after_key = i + quoted_key.len();
                let colon = contents[after_key..].find(':')? + after_key + 1;
                let value_start = contents[colon..].find('"')? + colon + 1;
                let value_end = value_start + contents[value_start..].find('"')?;
                let mut out = String::with_capacity(contents.len());
                out.push_str(&contents[..value_start]);
                out.push_str(new_value);
                out.push_str(&contents[value_end..]);
                return Some(out);
            }
            '"' => in_string = true,
            '{' | '[' => depth += 1,
            '}' | ']' => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    None
}

/// Replaces the `version` field inside the `[[package]]` block whose `name` matches
/// `package_name`, leaving every other byte of `Cargo.lock` untouched. Returns `None` if no
/// such block (or no version field inside it) is found. Relies on Cargo's own stable ordering
/// of `name` before `version` within a block.
fn replace_cargo_lock_package_version(
    contents: &str,
    package_name: &str,
    new_version: &str,
) -> Option<String> {
    let mut out = String::with_capacity(contents.len());
    let mut current_name: Option<&str> = None;
    let mut replaced = false;
    for line in contents.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let trimmed = body.trim();
        if trimmed == "[[package]]" {
            current_name = None;
        } else if !replaced {
            if let Some(name) = trimmed
                .strip_prefix("name = \"")
                .and_then(|s| s.strip_suffix('"'))
            {
                current_name = Some(name);
            } else if current_name == Some(package_name)
                && trimmed.strip_prefix("version = \"").is_some()
            {
                let indent = &body[..body.len() - body.trim_start().len()];
                let ending = &line[body.len()..];
                out.push_str(indent);
                out.push_str("version = \"");
                out.push_str(new_version);
                out.push('"');
                out.push_str(ending);
                replaced = true;
                continue;
            }
        }
        out.push_str(line);
    }
    replaced.then_some(out)
}

/// After a Cargo.toml version bump, Cargo's own lockfile must carry the same version in the
/// crate's `[[package]]` entry (Cargo rewrites it automatically on the next build otherwise,
/// which is exactly the kind of package-manager-required lockfile update CLAUDE.md allows).
/// Leaving it out of sync means a CI `cargo publish`/`cargo check` step finds an unexpectedly
/// dirty `Cargo.lock` right when it runs. No-ops if there's no lockfile or no matching entry.
fn sync_cargo_lock(repo_root: &Path, package_name: &str, new_version: &str) -> Result<()> {
    let lock_path = repo_root.join("Cargo.lock");
    if !lock_path.is_file() {
        return Ok(());
    }
    let contents = fs::read_to_string(&lock_path)
        .with_context(|| format!("failed to read {}", lock_path.display()))?;
    if let Some(updated) = replace_cargo_lock_package_version(&contents, package_name, new_version)
    {
        fs::write(&lock_path, updated)
            .with_context(|| format!("failed to write {}", lock_path.display()))?;
    }
    Ok(())
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

    fn write(&self, contents: &str, _current: &str, new_version: &str) -> Result<String> {
        replace_json_top_level_string(contents, "version", new_version)
            .context("could not find a top-level \"version\" field to update in package.json")
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

    fn write(&self, contents: &str, _current: &str, new_version: &str) -> Result<String> {
        replace_toml_string_in_table(contents, "package", "version", new_version)
            .context("could not find [package] version to update in Cargo.toml")
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

    fn write(&self, contents: &str, _current: &str, new_version: &str) -> Result<String> {
        // Mirrors `extract`'s priority: prefer [project], fall back to [tool.poetry].
        replace_toml_string_in_table(contents, "project", "version", new_version)
            .or_else(|| {
                replace_toml_string_in_table(contents, "tool.poetry", "version", new_version)
            })
            .context("could not find a version field to update in pyproject.toml")
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

    fn write(&self, contents: &str, current: &str, new_version: &str) -> Result<String> {
        let pos = contents
            .find(current)
            .with_context(|| format!("could not find version '{current}' in {}", self.file))?;
        let mut updated = contents.to_string();
        updated.replace_range(pos..pos + current.len(), new_version);
        Ok(updated)
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

/// Writes `new_version` into the file `detected` was read from, preserving its existing
/// structure and formatting as much as practical (per CLAUDE.md's version management rules).
pub fn apply(detected: &DetectedVersion, new_version: &str) -> Result<()> {
    let file_name = detected
        .file
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("invalid version file path: {}", detected.file.display()))?;
    let provider = providers()
        .into_iter()
        .find(|p| p.file_name() == file_name)
        .with_context(|| format!("no version provider for {file_name}"))?;

    let contents = fs::read_to_string(&detected.file)
        .with_context(|| format!("failed to read {}", detected.file.display()))?;
    let updated = provider.write(&contents, &detected.version, new_version)?;
    fs::write(&detected.file, &updated)
        .with_context(|| format!("failed to write {}", detected.file.display()))?;

    if file_name == "Cargo.toml" {
        let repo_root = detected
            .file
            .parent()
            .with_context(|| format!("invalid version file path: {}", detected.file.display()))?;
        let value: toml::Value = toml::from_str(&updated).context("invalid Cargo.toml")?;
        if let Some(name) = value
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|v| v.as_str())
        {
            sync_cargo_lock(repo_root, name, new_version)?;
        }
    }

    Ok(())
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

    #[test]
    fn apply_updates_cargo_toml_in_place_preserving_other_lines() {
        let dir = temp_dir();
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"x\"\nversion = \"0.4.0\"\nedition = \"2024\"\n\n[dependencies]\nversion = \"9.9.9\"\n",
        )
        .unwrap();
        let detected = detect(&dir).unwrap().unwrap();

        apply(&detected, "0.5.0").unwrap();

        let contents = fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        assert_eq!(
            contents,
            "[package]\nname = \"x\"\nversion = \"0.5.0\"\nedition = \"2024\"\n\n[dependencies]\nversion = \"9.9.9\"\n"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn apply_updates_pyproject_project_table() {
        let dir = temp_dir();
        fs::write(
            dir.join("pyproject.toml"),
            "[project]\nname = \"x\"\nversion = \"2.0.0\"\n",
        )
        .unwrap();
        let detected = detect(&dir).unwrap().unwrap();

        apply(&detected, "3.0.0").unwrap();

        let contents = fs::read_to_string(dir.join("pyproject.toml")).unwrap();
        assert_eq!(contents, "[project]\nname = \"x\"\nversion = \"3.0.0\"\n");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn apply_updates_pyproject_poetry_table_when_no_project_version() {
        let dir = temp_dir();
        fs::write(
            dir.join("pyproject.toml"),
            "[tool.poetry]\nname = \"x\"\nversion = \"2.0.0\"\n",
        )
        .unwrap();
        let detected = detect(&dir).unwrap().unwrap();

        apply(&detected, "3.0.0").unwrap();

        let contents = fs::read_to_string(dir.join("pyproject.toml")).unwrap();
        assert_eq!(
            contents,
            "[tool.poetry]\nname = \"x\"\nversion = \"3.0.0\"\n"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn apply_updates_only_the_top_level_version_in_package_json() {
        let dir = temp_dir();
        fs::write(
            dir.join("package.json"),
            "{\n  \"name\": \"x\",\n  \"version\": \"1.2.3\",\n  \"dependencies\": {\n    \"y\": \"version\"\n  }\n}",
        )
        .unwrap();
        let detected = detect(&dir).unwrap().unwrap();

        apply(&detected, "1.3.0").unwrap();

        let contents = fs::read_to_string(dir.join("package.json")).unwrap();
        assert_eq!(
            contents,
            "{\n  \"name\": \"x\",\n  \"version\": \"1.3.0\",\n  \"dependencies\": {\n    \"y\": \"version\"\n  }\n}"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn apply_syncs_the_matching_cargo_lock_entry() {
        let dir = temp_dir();
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"x\"\nversion = \"0.4.0\"\n",
        )
        .unwrap();
        fs::write(
            dir.join("Cargo.lock"),
            "[[package]]\nname = \"other\"\nversion = \"1.0.0\"\n\n[[package]]\nname = \"x\"\nversion = \"0.4.0\"\n",
        )
        .unwrap();
        let detected = detect(&dir).unwrap().unwrap();

        apply(&detected, "0.5.0").unwrap();

        let lock = fs::read_to_string(dir.join("Cargo.lock")).unwrap();
        assert_eq!(
            lock,
            "[[package]]\nname = \"other\"\nversion = \"1.0.0\"\n\n[[package]]\nname = \"x\"\nversion = \"0.5.0\"\n"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn apply_updates_plain_version_files() {
        let dir = temp_dir();
        fs::write(dir.join("VERSION"), "5.6.7\n").unwrap();
        let detected = detect(&dir).unwrap().unwrap();

        apply(&detected, "5.7.0").unwrap();

        let contents = fs::read_to_string(dir.join("VERSION")).unwrap();
        assert_eq!(contents, "5.7.0\n");
        fs::remove_dir_all(&dir).unwrap();
    }
}

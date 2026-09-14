# Autoship

From staged changes to pushed code in one command.

```bash
git add <files>
autoship
```

Autoship inspects your staged changes, recommends a version bump, suggests a commit message and branch name, and pushes the result, all while showing you the plan and letting you confirm, edit, or skip each step. The core workflow is fully deterministic: no AI required.

## Installation

```bash
cargo install autoship
```

Or build it from source:

```bash
git clone https://github.com/Chitresh-code/autoship
cd autoship
cargo install --path .
```

This requires the Rust toolchain (see [rustup.rs](https://rustup.rs)).

## Quick start

```bash
git add src/auth.rs
autoship
```

Autoship will:

1. Detect the repository, current branch, staged files, and remotes.
2. Detect the project version (if any) and suggest a SemVer bump based on the staged change.
3. Suggest a Conventional Commit message you can accept or edit.
4. Suggest a branch name and let you choose the current branch, the suggestion, or a custom name.
5. Commit, then push to a remote you select, once you confirm.

Nothing is committed, branched, or pushed without your confirmation.

## Example workflow

```text
✓ Git repository detected
✓ On branch main
✓ 1 staged file
✓ origin remote detected
✓ Rust project detected

Changes

  + src/auth.rs

Current version: 1.2.0
Suggested version: 1.3.0 (minor)

Suggested commit:

feat(auth): add token refresh

Suggested branch: feature/add-token-refresh
```

Run `autoship --dry-run` to see this plan without changing anything.

## Configuration

Autoship reads project configuration from `.autoship.toml` in the repository root, merged over global configuration from the platform's user configuration directory (for example `~/.config/autoship/config.toml` on Linux). Project settings override global ones.

```toml
[branch.prefixes]
feat = "feature/"
fix = "fix/"

[git]
remote = "upstream"
```

* `[branch.prefixes]` overrides the default branch prefix for a Conventional Commit type.
* `[git] remote` sets the preferred remote to push to, used instead of `origin`.

Both are optional; Autoship works with no configuration file at all.

## Supported ecosystems

Autoship detects and updates versions for:

* Node.js (`package.json`)
* Rust (`Cargo.toml`)
* Python (`pyproject.toml`)
* Generic (`VERSION` or `version.txt`)

## Development

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

See [CLAUDE.md](CLAUDE.md) for the project's engineering conventions.

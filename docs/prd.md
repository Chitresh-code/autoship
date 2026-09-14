# Ship CLI

## Product and Technical Specification

### 1. Product Overview

Ship is a developer-focused CLI that automates the repetitive workflow between finishing a feature and getting the changes pushed to a Git remote.

The intended workflow is simple:

```text
Implement feature
      ↓
git add .
      ↓
ship
      ↓
Analyze changes
      ↓
Version
      ↓
Branch
      ↓
Commit
      ↓
Push
      ↓
Optional Pull Request
```

The goal is to reduce repetitive Git and release-management work while keeping the developer in control of every meaningful repository change.

The CLI should work through a single primary command:

```bash
ship
```

The developer stages their changes, runs `ship`, reviews the proposed actions, and confirms the workflow.

---

# 2. Problem

After implementing a feature, developers often perform a sequence of repetitive tasks:

1. Inspect the staged changes.
2. Determine whether the version needs to change.
3. Decide whether the change represents a patch, minor, or major release.
4. Update version files manually.
5. Write a commit message.
6. Decide whether to stay on the current branch.
7. Create a feature, fix, docs, or refactor branch.
8. Rename or construct the branch.
9. Commit the changes.
10. Determine the correct Git remote.
11. Push the branch.
12. Optionally create a Pull Request.
13. Potentially generate a changelog or release.

AI assistants reduce some of this work, but they often require repeated prompting and manual execution of the resulting commands.

Ship turns this entire workflow into one repeatable CLI interaction.

---

# 3. Product Vision

Ship should become a developer's final step after completing a piece of work.

The desired workflow is:

```bash
git add .
ship
```

The developer should not need to remember:

```bash
git checkout -b feature/...
git commit -m "..."
git push -u origin ...
```

The CLI should inspect the repository, understand the project conventions, propose a workflow, and execute the approved plan.

---

# 4. Core Principles

## 4.1 One command

The common workflow should require:

```bash
ship
```

Advanced commands should exist, but the default experience should remain simple.

## 4.2 User control

Ship should never silently perform destructive Git operations.

The user should see the proposed plan before execution.

Example:

```text
Ship Plan

Version
  1.8.2 → 1.9.0

Branch
  main → feature/optional-token-config

Commit
  feat(auth): add optional token configuration

Remote
  origin

Push
  origin/feature/optional-token-config

? Execute this plan? [Y/n]
```

## 4.3 Deterministic first

AI should improve the experience, not become a hard dependency.

The core workflow should work without an AI provider.

## 4.4 Convention aware

Ship should inspect the repository before making recommendations.

Examples:

* Existing branch naming conventions
* Conventional Commit usage
* Version management strategy
* Git remote configuration
* Package ecosystem
* Existing project configuration

## 4.5 Extensible

The architecture should support additional ecosystems and workflows without requiring changes to the core workflow engine.

---

# 5. Example User Experience

The developer stages changes:

```bash
git add .
```

Then runs:

```bash
ship
```

Ship analyzes the repository.

```text
$ ship

Analyzing staged changes...

✓ Git repository detected
✓ 8 staged files
✓ TypeScript project detected
✓ package.json detected
✓ Conventional Commits detected
✓ origin remote detected

Changes

  M src/auth/config.ts
  M src/auth/client.ts
  A src/auth/token.ts
  M tests/auth.test.ts

Change analysis

  Type: feature
  Scope: auth
  Summary: add optional token configuration

Suggested version

  Current: 1.8.2
  Suggested: 1.9.0
  Reason: backwards-compatible functionality added

? Update version to 1.9.0? [Y/n/custom]
```

The user confirms.

```text
✓ package.json updated
```

Ship then proposes a branch.

```text
? Branch strategy

  1. Current branch
  2. Create feature branch
  3. Create fix branch
  4. Create docs branch
  5. Custom branch

> 2

Suggested branch:

  feature/optional-token-config

? Use this branch? [Y/n/edit]
```

Then the commit.

```text
Suggested commit:

feat(auth): add optional token configuration

? Use this commit? [Y/n/edit]
```

Finally:

```text
Ship Plan

✓ Version 1.8.2 → 1.9.0
✓ Branch feature/optional-token-config
✓ Commit feat(auth): add optional token configuration
✓ Remote origin
✓ Push origin/feature/optional-token-config

? Execute? [Y/n]
```

After execution:

```text
✓ Branch created
✓ Version updated
✓ Changes committed
✓ Branch pushed
✓ Upstream configured

Done.
```

---

# 6. MVP Features

## 6.1 Repository Detection

Ship should detect:

* Whether the current directory is a Git repository
* Current branch
* Repository status
* Staged files
* Unstaged files
* Git remotes
* Tracking branches
* Default branch
* Git provider where identifiable

Primary command:

```bash
ship
```

---

# 7. Staged Change Analysis

Ship should require or strongly recommend staged changes.

Example:

```text
$ ship

No staged changes found.

Stage your changes first:

  git add <files>

Then run:

  ship
```

If both staged and unstaged changes exist:

```text
You have unstaged changes.

Staged:
  src/auth.ts
  tests/auth.test.ts

Unstaged:
  README.md
  src/config.ts

Only staged changes will be included.

Continue? [y/N]
```

Ship should never accidentally include unstaged changes.

---

# 8. Version Management

Version management should support multiple project ecosystems.

## 8.1 JavaScript / Node.js

Detect:

```text
package.json
package-lock.json
yarn.lock
pnpm-lock.yaml
```

Primary version source:

```json
{
  "version": "1.8.2"
}
```

## 8.2 Rust

Detect:

```text
Cargo.toml
Cargo.lock
```

Example:

```toml
version = "1.8.2"
```

## 8.3 Python

Detect:

```text
pyproject.toml
setup.py
__version__
```

## 8.4 Generic version files

Support:

```text
VERSION
version.txt
```

Additional ecosystems should be added through providers.

---

# 9. Version Recommendation

Ship should analyze the staged changes and recommend a version bump.

Initial rules:

| Change                           | Suggested Version |
| -------------------------------- | ----------------- |
| Breaking API change              | Major             |
| New backwards-compatible feature | Minor             |
| Bug fix                          | Patch             |
| Documentation only               | No bump           |
| Internal refactor                | Patch or no bump  |
| Tests only                       | No bump           |
| Chore                            | No bump           |

Example:

```text
Current version: 1.8.2

Detected change:
  New backwards-compatible authentication functionality.

Suggested:
  1.9.0

? Apply version 1.9.0? [Y/n/custom/skip]
```

The user should always have the ability to specify a different version.

---

# 10. Commit Generation

Ship should support Conventional Commits.

Supported types:

```text
feat
fix
docs
refactor
perf
test
build
ci
chore
revert
```

Example:

```text
feat(auth): add optional token configuration
```

The CLI should provide:

```text
Suggested commit:

feat(auth): add optional token configuration

? Use this message? [Y/n/edit]
```

The user should be able to edit the message before committing.

---

# 11. Branch Management

Ship should recommend branch names based on the detected change.

Examples:

```text
feat(auth): add OAuth login
→ feature/oauth-login
```

```text
fix(config): handle missing environment variables
→ fix/handle-missing-environment-variables
```

```text
docs: update architecture documentation
→ docs/update-architecture
```

```text
refactor(api): simplify request handling
→ refactor/simplify-request-handling
```

Default prefixes:

```toml
[branch.prefixes]
feature = "feature/"
fix = "fix/"
docs = "docs/"
refactor = "refactor/"
chore = "chore/"
```

These should be configurable.

---

# 12. Branch Strategy

The user should choose between:

```text
Current branch
New feature branch
New fix branch
New docs branch
New refactor branch
Custom branch
```

Example:

```text
Current branch:
  main

? Where should these changes go?

  1. main
  2. feature/optional-token-config
  3. Custom branch

> 2
```

Ship should check whether a suggested branch already exists.

If it exists:

```text
Branch already exists:

feature/optional-token-config

? Switch to existing branch? [Y/n]
```

---

# 13. Remote Detection

Ship should inspect:

```bash
git remote -v
```

Example:

```text
origin    git@github.com:user/project.git
upstream  git@github.com:company/project.git
```

If only one remote exists:

```text
Remote:
  origin

Use origin? [Y/n]
```

If multiple remotes exist:

```text
? Push to:

  1. origin
  2. upstream
  3. Custom remote
```

The selected remote should be recorded in the execution plan.

---

# 14. Push Workflow

For a new branch:

```bash
git push -u origin feature/oauth-login
```

Ship should configure upstream tracking automatically.

Expected output:

```text
✓ Pushed origin/feature/oauth-login
✓ Upstream configured
```

For an existing tracking branch:

```text
✓ Pushed origin/feature/oauth-login
```

---

# 15. Dry Run

Ship should support:

```bash
ship --dry-run
```

Example:

```text
Ship Plan

Version:
  1.8.2 → 1.9.0

Branch:
  main → feature/oauth-login

Commit:
  feat(auth): add OAuth login

Remote:
  origin

Push:
  origin/feature/oauth-login

No changes were made.
```

This should be safe to run repeatedly.

---

# 16. Non-Interactive Mode

Ship should support:

```bash
ship --yes
```

This mode should use the generated plan without interactive confirmation.

It should be suitable for advanced users and automation.

Potential future options:

```bash
ship --version patch
ship --version minor
ship --version major
ship --no-version
ship --branch current
ship --branch feature
ship --remote origin
ship --no-ai
```

---

# 17. AI Integration

AI should remain optional.

Possible providers:

```text
OpenAI
Anthropic
Ollama
OpenAI-compatible APIs
Local models
```

AI should receive the minimum required context.

Example:

```json
{
  "current_branch": "main",
  "current_version": "1.8.2",
  "project_type": "typescript",
  "commit_convention": "conventional",
  "staged_diff": "..."
}
```

AI should return structured data.

Example:

```json
{
  "change_type": "feat",
  "scope": "auth",
  "summary": "add optional token configuration",
  "version_bump": "minor",
  "branch_name": "feature/optional-token-config"
}
```

The Rust application should validate the result before using it.

AI proposes.

Ship validates and executes.

---

# 18. Configuration

Ship should support a project configuration file:

```text
.ship.toml
```

Example:

```toml
[ship]
version = true
branch = true
commit = true
push = true

[version]
strategy = "semver"

[commit]
conventional = true
ai = true

[branch]
enabled = true

[branch.prefixes]
feature = "feature/"
fix = "fix/"
docs = "docs/"
refactor = "refactor/"
chore = "chore/"

[git]
remote = "origin"
```

Global configuration should also be supported.

Example:

```text
~/.config/ship/config.toml
```

Project configuration should override global configuration.

---

# 19. Architecture

Ship should use a modular Rust architecture.

Suggested workspace:

```text
ship/
├── Cargo.toml
├── crates/
│   ├── ship-cli/
│   ├── ship-core/
│   ├── ship-git/
│   ├── ship-version/
│   ├── ship-ai/
│   ├── ship-config/
│   └── ship-github/
├── tests/
├── docs/
└── README.md
```

## Core components

### ship-cli

Responsible for:

* CLI arguments
* Terminal UI
* User prompts
* Output formatting

### ship-core

Responsible for:

* Workflow orchestration
* Ship plan
* Validation
* Execution lifecycle

### ship-git

Responsible for:

* Git status
* Branches
* Commits
* Remotes
* Push operations

### ship-version

Responsible for:

* Version detection
* Version parsing
* Version bumping
* Ecosystem providers

### ship-ai

Responsible for:

* AI provider abstraction
* Prompt construction
* Structured responses
* Provider configuration

### ship-config

Responsible for:

* Global configuration
* Project configuration
* Configuration merging
* Defaults

### ship-github

Responsible for future:

* Pull Requests
* GitHub releases
* GitHub metadata

---

# 20. Core Data Model

The workflow should produce a plan before executing changes.

Conceptually:

```rust
struct ShipPlan {
    version_change: Option<VersionChange>,
    branch: BranchAction,
    commit: CommitAction,
    remote: Remote,
    push: PushAction,
}
```

Example:

```rust
struct VersionChange {
    current: String,
    proposed: String,
    strategy: String,
}

struct BranchAction {
    current: String,
    target: String,
    create: bool,
}

struct CommitAction {
    message: String,
    conventional_type: String,
    scope: Option<String>,
}

struct Remote {
    name: String,
    url: String,
}
```

This separates analysis from execution.

---

# 21. Workflow Engine

The execution pipeline should follow:

```text
Detect
  ↓
Analyze
  ↓
Recommend
  ↓
Build Plan
  ↓
Validate
  ↓
Confirm
  ↓
Execute
  ↓
Verify
```

The system should not mutate the repository during analysis.

This enables:

```bash
ship --dry-run
```

and gives the user a clear execution plan.

---

# 22. Error Handling

Errors should explain:

1. What failed.
2. What Ship already changed.
3. What remains.
4. How the user should recover.

Example:

```text
✗ Push failed

The branch and commit were created successfully.

Branch:
  feature/oauth-login

Commit:
  a81f23c

Push failed because the remote rejected the operation.

Try:

  git push -u origin feature/oauth-login
```

Ship should avoid leaving the user unsure about repository state.

---

# 23. Security

Ship interacts directly with source code and Git credentials.

Security requirements:

* Never print credentials.
* Never expose Git tokens.
* Avoid sending the complete repository to AI providers.
* Send only the staged diff and required metadata.
* Make AI providers explicit.
* Provide a fully local mode.
* Never execute arbitrary AI-generated shell commands.
* Validate AI-generated structured responses.
* Require confirmation before destructive actions.

AI output should never become unrestricted shell execution.

---

# 24. Distribution

The primary application should be a native Rust binary.

Supported platforms:

```text
macOS
Linux
Windows
```

Distribution options:

```text
GitHub Releases
Homebrew
cargo install
npm wrapper
```

The npm package should optionally provide:

```bash
npm install -g @ship/cli
```

while downloading or invoking the appropriate native binary.

This gives Node.js developers a familiar installation method without making Node.js a runtime dependency.

---

# 25. CLI Interface

Primary command:

```bash
ship
```

Potential commands:

```bash
ship init
ship config
ship doctor
ship release
ship pr
ship version
ship branch
ship commit
```

Flags:

```bash
ship --dry-run
ship --yes
ship --no-ai
ship --verbose
ship --version
```

The initial release should keep the command surface small.

---

# 26. Future GitHub Workflow

A future command:

```bash
ship --pr
```

could execute:

```text
Analyze changes
      ↓
Version
      ↓
Create branch
      ↓
Commit
      ↓
Push
      ↓
Generate PR title
      ↓
Generate PR description
      ↓
Create PR
```

Example PR:

```text
Title:
feat(auth): add OAuth login

Summary:
- Add OAuth login flow
- Add token persistence
- Add authentication tests

Testing:
- Unit tests
- Integration tests
```

---

# 27. Future Release Workflow

A future release command:

```bash
ship release
```

could handle:

```text
Version bump
      ↓
CHANGELOG
      ↓
Commit
      ↓
Git tag
      ↓
Push tag
      ↓
GitHub Release
      ↓
Package publishing
```

Potential package ecosystems:

```text
npm
crates.io
PyPI
Go
GitHub Releases
```

This should remain outside the initial MVP.

---

# 28. Monorepo Support

Future versions should support:

```text
apps/
packages/
crates/
services/
```

Example:

```text
packages/auth/package.json
packages/ui/package.json
packages/api/Cargo.toml
```

Ship should identify which packages changed and determine whether:

* One package needs a version bump.
* Multiple packages need version bumps.
* Workspace versions need synchronized updates.

---

# 29. Open Source Strategy

The core CLI should be open source.

Suggested repository:

```text
github.com/<organization>/ship
```

Recommended license options:

```text
MIT
Apache-2.0
```

Apache-2.0 is a strong option if you want explicit patent protections.

The repository should contain:

```text
README.md
CONTRIBUTING.md
LICENSE
SECURITY.md
CHANGELOG.md
docs/
examples/
```

---

# 30. Commercial Strategy

The open-source CLI should provide substantial value without requiring payment.

Potential paid functionality:

## Ship Cloud

```text
Team configuration
Shared workflow policies
Organization conventions
Central AI configuration
Usage analytics
Audit logs
Release dashboards
Enterprise authentication
```

## Hosted AI

Users who do not want to configure their own AI provider could use a hosted service.

## Team Workflows

Organizations could enforce:

```text
Branch naming
Commit conventions
Version policies
PR templates
Release policies
```

Example:

```toml
[policy]
require_conventional_commits = true
require_branch_prefix = true
require_version_check = true
require_pr = true
```

---

# 31. Product Differentiation

Ship should differentiate itself through workflow quality rather than simply generating commit messages.

The value proposition is:

```text
Understand my repository.
Understand my changes.
Propose the correct workflow.
Ask me once.
Execute it safely.
```

The product should feel like a repository-aware shipping assistant.

---

# 32. MVP Roadmap

## v0.1

Focus on the core workflow.

### Git

* Detect repository
* Detect staged changes
* Detect current branch
* Detect remotes
* Create branches
* Commit changes
* Push changes

### Versioning

* package.json
* Cargo.toml
* pyproject.toml
* VERSION
* SemVer

### Commit

* Conventional Commit detection
* Commit message recommendation
* Manual editing

### Branching

* Feature
* Fix
* Docs
* Refactor
* Chore
* Custom

### Safety

* Dry run
* Confirmation
* Unstaged change detection
* Execution summary

---

# 33. v0.2

Add:

* GitHub integration
* Pull Request creation
* GitLab integration
* CHANGELOG generation
* Git tags
* Better project detection
* Monorepo basics

---

# 34. v0.3

Add:

* AI provider abstraction
* OpenAI
* Anthropic
* Ollama
* Local models
* Improved change classification
* AI-generated commit messages
* AI-generated branch names
* AI-assisted version recommendations

---

# 35. v1.0

Target experience:

```bash
git add .
ship
```

The CLI should reliably handle:

```text
Repository detection
Change analysis
Version recommendation
Version modification
Branch recommendation
Branch creation
Commit generation
Commit creation
Remote detection
Push
Optional PR creation
```

with strong safety guarantees.

---

# 36. Success Metrics

Potential adoption metrics:

```text
Weekly active users
Monthly active repositories
Successful ship operations
Average workflow duration
GitHub stars
CLI downloads
Package installations
AI usage rate
PR creation rate
```

The most important product metric should be:

> Percentage of users who run `ship` after staging changes and complete the workflow successfully.

---

# 37. Example README Positioning

Ship's README should communicate the product in a few seconds.

````text
# Ship

From staged changes to pushed code in one command.

```bash
git add .
ship
````

Ship analyzes your changes, recommends the version,
creates the right branch, writes the commit, and pushes
your code.

You review the plan before anything changes.

✓ Version management
✓ Smart branch names
✓ Conventional commits
✓ Git remote detection
✓ Safe execution
✓ AI optional
✓ Rust powered

````

---

# 38. Long-Term Vision

Ship should evolve from a Git workflow helper into a general developer shipping workflow engine.

The long-term workflow could become:

```text
Code
 ↓
Stage
 ↓
Analyze
 ↓
Version
 ↓
Branch
 ↓
Commit
 ↓
Push
 ↓
PR
 ↓
Review
 ↓
Merge
 ↓
Release
 ↓
Publish
````

The initial product should solve the first half extremely well before expanding into releases, CI, publishing, and team workflows.

---

# 39. Recommended First Implementation

Build the first version in this order:

```text
1. Rust CLI
2. Git repository inspection
3. Staged diff extraction
4. Version detection
5. SemVer calculation
6. Interactive version confirmation
7. Commit message generation
8. Branch name generation
9. Interactive workflow plan
10. Branch creation
11. Commit creation
12. Remote detection
13. Push
14. Dry-run mode
15. Configuration
16. Tests
17. GitHub Actions
18. Public release
```

Do not start with AI.

First make this workflow excellent:

```bash
git add .
ship
```

Then add AI where it produces a measurable improvement.

---

# 40. Core Product Statement

Ship is a Rust-based CLI that turns staged Git changes into a clean, reviewable shipping workflow.

Instead of manually deciding how to version, branch, commit, and push every change, developers run:

```bash
ship
```

Ship analyzes the repository, proposes the appropriate actions, asks for confirmation, and executes the workflow safely.

The core product promise is:

```text
Stage your work.
Run Ship.
Review the plan.
Ship the code.
```

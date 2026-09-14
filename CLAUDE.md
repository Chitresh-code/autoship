# CLAUDE.md

Instructions for Claude and any contributor working in this repository.

## Project

Autoship is a Rust-based CLI that automates the workflow between staged Git changes and pushed code.

Autoship should make common Git shipping workflows fast, predictable, and safe.

The CLI should remain useful without AI. AI provides recommendations and additional intelligence, but the core Git workflow must remain deterministic.

## Rust tooling

Use Cargo for dependency and build management.

Use the Rust toolchain configured by the repository.

Before adding a dependency, check whether the standard library or an existing dependency already solves the problem.

Prefer small, well-maintained dependencies.

Do not add dependencies for trivial functionality.

Run formatting with:

```bash
cargo fmt --all
```

Run checks with:

```bash
cargo check --workspace
```

Run tests with:

```bash
cargo test --workspace
```

Run Clippy with:

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

All production changes should pass formatting, compilation, tests, and Clippy.

## Git workflow

Never commit changes.

Never push changes.

Never create or delete branches unless the user explicitly asks Claude to perform that Git operation.

Claude is developing Autoship, not operating Autoship on behalf of the user.

When testing Git functionality, use temporary repositories created specifically for tests.

Do not run tests against the user's real repository when the test might modify Git state.

## Git safety

Git operations are potentially destructive.

Never assume repository state.

Inspect before acting.

At minimum, inspect:

* Repository root
* Current branch
* Repository status
* Staged files
* Unstaged files
* Git remotes
* Upstream tracking information
* Relevant Git configuration

Never use `git add -A` or `git add .` internally to collect changes.

Autoship operates on changes the user already staged.

Never accidentally include unstaged changes.

Do not reset, clean, checkout with destructive behavior, rebase, force push, or delete branches as part of the normal workflow.

Never force push.

Never rewrite existing commits as part of the standard workflow.

## Staged changes

The staged diff is the primary source of truth for change analysis.

Use the staged diff when generating:

* Change classification
* Commit message
* Branch name
* Version recommendation

Do not analyze the entire working tree when staged changes are available.

If there are unstaged changes, clearly tell the user.

Example:

```text
Staged:
  src/auth.ts
  tests/auth.test.ts

Unstaged:
  README.md
  src/config.ts

Only staged changes will be included.
```

## Version management

Version management must be provider based.

Initial providers:

* Node.js `package.json`
* Rust `Cargo.toml`
* Python `pyproject.toml`
* `VERSION`
* `version.txt`

Do not implement version detection as one large conditional function.

Use a provider abstraction.

Version changes must preserve the existing file structure and formatting as much as practical.

Do not modify unrelated files.

Do not update lockfiles unless the project's package manager requires the update.

Never silently change a version.

Always show:

```text
Current version: 1.8.2
Suggested version: 1.9.0
```

before applying a version change.

The user must be able to:

* Accept the suggestion
* Specify another version
* Skip versioning

Use SemVer where the project uses SemVer.

## Commit messages

Prefer Conventional Commits when the repository uses them or when Autoship is configured to use them.

Examples:

```text
feat(auth): add OAuth login
fix(config): handle missing environment variables
docs: update architecture documentation
refactor(api): simplify request handling
```

Commit messages should describe the staged change.

Do not generate generic messages such as:

```text
update code
fix stuff
changes
misc updates
```

Users should be able to edit the generated commit message before execution.

## Branch naming

Default branch naming:

```text
feature/short-title
fix/short-title
docs/short-title
refactor/short-title
chore/short-title
```

Branch naming must be configurable.

Suggested branch names should be:

* Short
* Descriptive
* Lowercase
* Hyphen separated
* Based on the actual staged change

Avoid unnecessarily long branch names.

Before creating a branch, check whether the branch already exists.

Never silently overwrite or replace an existing branch.

## Git remotes

Inspect configured remotes before pushing.

Do not assume the remote is named `origin`.

If multiple remotes exist, preserve enough information for Autoship to make a safe recommendation.

Never expose credentials or authentication tokens.

Never print credential-bearing remote URLs.

## AI

AI is optional.

The core workflow must work without an AI provider.

AI should recommend structured information such as:

```json
{
  "change_type": "feat",
  "scope": "auth",
  "summary": "add optional token configuration",
  "version_bump": "minor",
  "branch_name": "feature/optional-token-config"
}
```

Never execute arbitrary shell commands returned by an AI model.

AI output must pass schema validation and domain validation before Autoship uses it.

AI suggestions are not authoritative.

The user remains responsible for confirming the final plan.

Only send the minimum required repository context to an external AI provider.

The staged diff should be the primary code context.

## CLI UX

The terminal interface is part of the product.

Prefer concise, structured output.

Avoid unnecessary logs during normal operation.

Provide detailed diagnostics through verbose mode.

Errors should explain:

1. What failed.
2. What Autoship already changed.
3. What remains.
4. What the user should do next.

Do not leave the user uncertain about repository state.

## Non-interactive mode

Support a non-interactive mode for automation.

Example:

```bash
autoship --yes
```

Interactive prompts must not be required in non-interactive mode.

When required information is missing in non-interactive mode, fail clearly rather than guessing.

## Dry run

Support:

```bash
autoship --dry-run
```

Dry run should show the proposed workflow without modifying:

* Files
* Branches
* Commits
* Git configuration
* Remote state

## Configuration

Project configuration lives in:

```text
.autoship.toml
```

Global configuration should live in the platform-appropriate user configuration directory.

Project configuration overrides global configuration.

Do not introduce configuration options before there is a concrete use case.

Avoid speculative configuration.

## Testing

Git functionality must use temporary repositories in integration tests.

Tests should verify real Git behavior where practical.

Important scenarios include:

* Clean repository
* Staged changes
* Unstaged changes
* Staged and unstaged changes together
* No staged changes
* Existing branch
* New branch
* Multiple remotes
* Existing upstream
* Missing remote
* Push failure
* Version detection
* Version update
* Invalid version
* Dry run
* Non-interactive mode
* AI unavailable
* AI returns invalid structured output

Prefer integration tests for behavior involving Git.

Prefer unit tests for pure logic such as:

* Version calculations
* Branch naming
* Commit formatting
* Configuration merging
* Change classification

## Cross-platform support

Autoship is a native CLI.

Code should support:

* macOS
* Linux
* Windows

Do not assume Unix shell behavior.

Avoid shell-specific commands where Rust APIs or direct process execution provide a safer alternative.

Do not construct shell command strings when direct process arguments are available.

Test platform-specific behavior where practical.

## Dependencies

Prefer:

* `clap` for CLI parsing
* `serde` for serialization
* `toml` for configuration
* `semver` for SemVer
* `thiserror` for library errors
* `anyhow` for application-level error context
* `assert_cmd` for CLI integration tests

Use existing dependencies before introducing alternatives.

Do not add a dependency solely for convenience when a small amount of idiomatic Rust solves the problem cleanly.

## Code quality

Keep changes minimal and scoped.

Do not introduce speculative abstractions.

Do not add unused configuration.

Do not add unused traits or interfaces solely for hypothetical future functionality.

Write idiomatic Rust.

Prefer clear ownership and error handling over clever abstractions.

Avoid unnecessary cloning.

Avoid unnecessary allocation.

Keep public APIs small.

Document public APIs when their behavior is not obvious.

Production code must not contain placeholders, stubs, TODO implementations, or "good enough for now" shortcuts.

## Error handling

Errors should provide useful context.

Prefer typed errors at library boundaries.

Do not silently swallow errors.

Do not use `unwrap()` or `expect()` in production code unless the invariant is genuinely impossible to violate and the reason is clear from the surrounding code.

User-facing errors should be actionable.

## Documentation

Keep documentation close to the behavior it describes.

Important architectural decisions belong in `docs/`.

The README should focus on:

* What Autoship does
* Installation
* Quick start
* Example workflow
* Configuration
* Supported ecosystems
* Development

Do not put implementation details into the README unless contributors need them.

Never hard-wrap prose in documentation.

Write each paragraph as one logical line and let editors soft-wrap it.

## Formatting and style

Never use em dashes.

Use commas, colons, parentheses, or separate sentences instead.

Never use emojis in:

* Documentation
* Code
* Comments
* Commit messages
* CLI output

Avoid decorative separator lines such as:

```text
-----
=====
```

Use headings and whitespace instead.

## Pull Requests

When preparing a PR, describe:

* What changed
* Why it changed
* How it was tested
* Any compatibility considerations

Keep PRs focused.

Do not mix unrelated refactors with feature work.

## Development priorities

When choosing between implementation options, prioritize:

1. Safety
2. Correctness
3. Predictability
4. Cross-platform behavior
5. Good CLI UX
6. Maintainability
7. Performance

Do not optimize prematurely.

For Git operations, correctness and safety always take priority over speed.

## Definition of done

A feature is complete when:

* The implementation is production quality.
* The behavior is covered by appropriate tests.
* `cargo fmt --all` passes.
* `cargo check --workspace` passes.
* `cargo test --workspace` passes.
* `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes.
* Error cases are handled.
* Cross-platform behavior has been considered.
* User-facing CLI output is clear.
* Documentation is updated where needed.
* No unrelated files or behavior changed.

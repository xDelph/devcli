# Contributing to devcli

Thank you for your interest in contributing to devcli!

## Development Setup

See [DEVELOPMENT.md](DEVELOPMENT.md) for development environment setup.

## Commit Message Convention

We use **Conventional Commits** for automated versioning and changelog generation.

### Format

```
<type>(<scope>): <subject>

[optional body]

[optional footer]
```

### Examples

```bash
# Feature (bumps minor version)
feat(tui): add syntax highlighting to log viewer

# Bug fix (bumps patch version)
fix(monitor): prevent restart loop on health check failure

# Performance improvement (bumps patch version)
perf(startup): lazy-load configuration files

# Breaking change (bumps major version)
feat!: redesign configuration format

BREAKING CHANGE: Config schema changed. See migration guide in docs/

# Documentation (no version bump)
docs: update installation instructions

# Refactoring (no version bump)
refactor(process): simplify spawn logic
```

### Types

**Trigger version bump:**
- `feat:` - New feature (minor bump)
- `fix:` - Bug fix (patch bump)
- `perf:` - Performance improvement (patch bump)
- `revert:` - Revert previous commit (patch bump)

**Appear in changelog:**
- `docs:` - Documentation changes
- `refactor:` - Code refactoring

**Hidden from changelog:**
- `style:` - Code formatting
- `test:` - Adding tests
- `build:` - Build system changes
- `ci:` - CI/CD changes
- `chore:` - Other changes

### Scopes

Use scopes to organize changes:
- `config` - Configuration system
- `tui` - Terminal UI
- `monitor` - Process monitoring
- `process` - Process management
- `metrics` - Metrics system
- `cli` - CLI commands

### Breaking Changes

For breaking changes, use `!` after type or add `BREAKING CHANGE:` in footer:

```bash
feat(config)!: migrate configuration format

# or

feat(config): add support for per-project overrides

BREAKING CHANGE: The configuration file schema changed; run `devcli config migrate`
to upgrade an existing `~/.devcli/config.json`.
```

## Pull Request Process

1. Create a feature branch: `git checkout -b feat/my-feature`
2. Make your changes with conventional commits
3. Run tests: `cargo test --workspace`
4. Push and create PR
5. Wait for CI checks to pass
6. Request review

## Release Process

Releases are automated via Release Please:

1. Merge your PR to `develop`
2. Release Please analyzes commits
3. Creates a "Release PR" with:
   - Updated version in Cargo.toml
   - Generated CHANGELOG.md entries
4. Maintainer reviews and merges Release PR
5. Tag created automatically
6. A `Release` workflow builds every platform (macOS Intel/ARM, Linux x64/ARM),
   attaches the `devcli` and `pm-daemon` archives to the GitHub release, publishes
   it, then updates the Homebrew tap

## Code Style

- Run `cargo fmt` before committing
- Run `cargo clippy` and fix warnings
- Follow existing code patterns
- Add tests for new features

## Questions?

Open an issue or discussion on GitHub!

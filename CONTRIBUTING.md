# Contributing to devcli

Thank you for your interest in contributing to devcli!

## Development Setup

- **Rust 1.98.0** (edition 2024). `rust-toolchain.toml` pins it, with
  `rustfmt` and `clippy`; `rustup` installs it on the first `cargo` call.
- macOS or Linux: process management relies on Unix process groups and
  signals.
- Docker, OrbStack, kubectl, Node.js or Python are only needed to try the
  matching environments by hand, not to build or test.

```bash
cargo build                      # debug build: target/debug/devcli and pm-daemon
cargo run -p devcli -- --help    # run the CLI from source
cargo build --release            # optimized binaries in target/release/
```

`devcli` looks for `pm-daemon` on `PATH` or next to its own binary; a cargo
build puts both in the same `target/` directory.

### Workspace layout

| Crate | What it is |
|-------|------------|
| `devcli` | The `devcli` binary: argument parsing (`clap`) and dispatch to `devcli-core`. |
| `devcli-core` | Commands, TUI, worktrees, logging and metrics; the glue to the crates below (`*_support.rs`). |
| `app-detector` | Strategy-based detection of app types and of the commands each environment (local, Docker, OrbStack, Kubernetes, Nx) can run; also a standalone `app-detector` binary (`cli` feature). |
| `config-manager` | Schema-based configuration loading, validation and dependency resolution. |
| `env-flow` | Layered env-file loading (stage × runtime context) with interpolation. |
| `process-manager` | Process spawning, state tracking, health checks and the `pm-daemon` supervisor binary. |

### Checks

CI (`.github/workflows/ci.yml`) runs the following; run them before opening a
pull request:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo test -p app-detector --all-features
cargo test -p config-manager --all-features
cargo test -p env-flow --all-features
cargo test -p devcli-core --all-features
cargo test -p devcli --all-features
cargo test -p process-manager --all-features   # spawns and kills real processes
cargo test --workspace --doc
cargo deny check                                # advisories, licences, sources (deny.toml)
```

### Code guidelines

- Fallible code returns `Result` and adds context (`anyhow::Context`) where
  the error reaches the user; no `unwrap()` / `expect()` outside tests unless
  the invariant is local and obvious.
- Comments explain *why* (the scenario that broke, the constraint), not what
  the next line does. Public items get `///` docs.
- Every bug fix comes with a regression test next to the code it fixes
  (`#[cfg(test)]` modules or the crate's `tests/`).
- Generated commands are split with `shell_words`, not run through a shell:
  no `&&`, pipes or redirections in detected commands.
- New dependencies must pass `cargo deny check`; prefer the standard library
  or a crate already in `Cargo.lock`.

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

The sections are configured in `release-please-config.json`
(`changelog-sections`).

### Scopes

Use the crate or the area the change is about, for example `app-detector`,
`process-manager`, `pm-daemon`, `env-flow`, `config`, `cli`, `tui`,
`worktrees`, `auto-add`, `start`, `stop`, `logs`, `release`, `ci`.

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

1. Create a feature branch from `develop`: `git checkout -b feat/my-feature`
2. Make your changes with conventional commits
3. Run the [checks](#checks)
4. Push and create a PR against `develop`
5. Wait for CI checks to pass
6. Request review

## Release Process

Releases are automated via Release Please:

1. Merge your PR to `develop`
2. Once CI is green on `develop`, Release Please analyzes the commits
3. Creates a "Release PR" with:
   - Updated version in Cargo.toml (and Cargo.lock)
   - Generated CHANGELOG.md entries
4. Maintainer reviews and merges Release PR
5. Tag created automatically
6. A `Release` workflow builds every platform (macOS Intel/ARM, Linux x64/ARM),
   attaches the `devcli` and `pm-daemon` archives to the GitHub release, publishes
   it, then updates the Homebrew tap

The website version is not bumped by Release Please: `website/package.json`
and the pages showing the version are synced from `Cargo.toml` by
`scripts/sync-version.mjs` before every website build.

## Questions?

Open an issue or discussion on GitHub!

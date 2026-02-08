# GitHub Actions Setup - Complete ✅

## Summary

Automated CI/CD pipeline for testing, building, and releasing devcli across multiple platforms.

## Workflows Created

### 1. CI Workflow (`.github/workflows/ci.yml`)

**Triggers:**
- Push to `main` or `develop` branches
- Pull requests to `main` or `develop`

**Jobs:**

1. **Test** (Ubuntu + macOS)
   - Runs `cargo test --workspace --all-features`
   - Runs doc tests
   - Uses cargo caching for speed

2. **Lint** (Ubuntu)
   - `cargo fmt --all -- --check` (formatting)
   - `cargo clippy` (linting with warnings as errors)

3. **Build** (Ubuntu + macOS + Windows)
   - `cargo build --release`
   - Verifies binary exists

**Purpose:** Ensures code quality on every commit and PR

### 2. Release Workflow (`.github/workflows/release.yml`)

**Triggers:**
- Push tags matching `v*.*.*` (e.g., `v0.1.0`, `v1.2.3`)

**Jobs:**

1. **Create Release**
   - Extracts version from tag
   - Creates GitHub release with installation instructions
   - Provides upload URL for binaries

2. **Build Release** (Matrix: 5 platforms)
   - Builds for:
     - macOS Intel (`x86_64-apple-darwin`)
     - macOS Apple Silicon (`aarch64-apple-darwin`)
     - Linux x64 (`x86_64-unknown-linux-gnu`)
     - Linux ARM64 (`aarch64-unknown-linux-gnu`)
     - Windows x64 (`x86_64-pc-windows-msvc`)
   - Strips binaries (Unix)
   - Creates `.tar.gz` (Unix) or `.zip` (Windows)
   - Generates SHA256 checksums
   - Uploads to GitHub release

**Purpose:** Automated multi-platform releases with one command

## Platform Support

| Platform | Architecture | Binary Format | Cross-compile |
|----------|-------------|---------------|---------------|
| macOS | Intel (x86_64) | .tar.gz | No |
| macOS | Apple Silicon (ARM64) | .tar.gz | No |
| Linux | x86_64 | .tar.gz | No |
| Linux | ARM64 | .tar.gz | Yes (via cross) |
| Windows | x86_64 | .zip | No |

## Usage

### Running CI

CI runs automatically on push/PR. To trigger manually:

1. Go to Actions tab on GitHub
2. Select "CI" workflow
3. Click "Run workflow"

### Creating a Release

```bash
# 1. Update version in Cargo.toml
# [workspace.package]
# version = "0.2.0"

# 2. Commit changes
git add Cargo.toml CHANGELOG.md
git commit -m "chore: bump version to 0.2.0"

# 3. Create and push tag
git tag -a v0.2.0 -m "Release v0.2.0"
git push origin v0.2.0

# 4. Watch GitHub Actions build and release
```

The release workflow will:
- Build binaries for all 5 platforms (~10-15 minutes)
- Create GitHub release
- Upload all binaries and checksums
- Add installation instructions

### Verifying Release

```bash
# Download and verify (macOS example)
curl -LO https://github.com/YOUR_USERNAME/devcli/releases/download/v0.2.0/devcli-x86_64-apple-darwin.tar.gz
curl -LO https://github.com/YOUR_USERNAME/devcli/releases/download/v0.2.0/devcli-x86_64-apple-darwin.tar.gz.sha256

# Verify checksum
shasum -a 256 -c devcli-x86_64-apple-darwin.tar.gz.sha256

# Extract and test
tar xzf devcli-x86_64-apple-darwin.tar.gz
./devcli --version
```

## Caching Strategy

Both workflows use aggressive caching to speed up builds:

- **Cargo registry cache**: `~/.cargo/registry`
- **Cargo index cache**: `~/.cargo/git`
- **Build cache**: `target/` directory

Cache keys include:
- OS (`${{ runner.os }}`)
- Target triple (for releases)
- `Cargo.lock` hash

This reduces build times from ~5 minutes to ~1-2 minutes on cache hit.

## Secrets Required

No additional secrets needed! Uses `GITHUB_TOKEN` (automatically provided).

## Customization

### Add More Platforms

Edit `.github/workflows/release.yml` matrix:

```yaml
matrix:
  include:
    # Add new platform
    - os: ubuntu-latest
      target: x86_64-unknown-linux-musl
      use_cross: true
```

### Change Trigger Branches

Edit `.github/workflows/ci.yml`:

```yaml
on:
  push:
    branches: [ main, develop, staging ]  # Add more branches
```

### Add Pre-release Support

Tags like `v0.2.0-alpha.1` will trigger release, but you can mark as pre-release:

```yaml
- name: Create Release
  with:
    prerelease: ${{ contains(github.ref, 'alpha') || contains(github.ref, 'beta') }}
```

## Troubleshooting

### CI Fails on Formatting

```bash
# Locally run and fix
cargo fmt --all
git add .
git commit -m "fix: formatting"
```

### CI Fails on Clippy

```bash
# Locally run and fix
cargo clippy --workspace --all-features --all-targets -- -D warnings

# Fix issues, then commit
```

### Release Build Fails

1. Check Actions logs for specific error
2. Common issues:
   - Compilation errors (fix code)
   - Missing dependencies (update Cargo.toml)
   - Cross-compilation issues (check `cross` setup)

### Re-doing a Release

```bash
# Delete tag locally and remotely
git tag -d v0.2.0
git push origin :refs/tags/v0.2.0

# Delete GitHub release via web UI

# Fix issues, re-tag, and push
git tag -a v0.2.0 -m "Release v0.2.0"
git push origin v0.2.0
```

## Files Overview

```
.github/
├── workflows/
│   ├── ci.yml          # Continuous Integration
│   └── release.yml     # Release Automation
docs/
├── release-process.md  # Detailed release guide
└── github-actions-setup.md  # This file
CHANGELOG.md           # Version history
README.md              # Updated with badges and installation
```

## Next Steps

1. **First Release**: Create `v0.1.0` tag to test workflow
2. **Badge URLs**: Update `YOUR_USERNAME` in README.md badges
3. **Optional**:
   - Add code coverage (codecov.io)
   - Add dependency scanning (dependabot)
   - Add security audits (cargo-audit)
   - Publish to crates.io

## Status

✅ CI workflow configured and ready
✅ Release workflow configured and ready
✅ Multi-platform builds (5 platforms)
✅ Automatic binary uploads
✅ Checksum generation
✅ Documentation complete

**Ready to create first release!**

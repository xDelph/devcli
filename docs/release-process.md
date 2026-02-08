# Release Process

This document describes how to create a new release of devcli.

## Overview

Releases are automated via GitHub Actions. When you push a version tag, the release workflow:

1. Runs all tests
2. Builds binaries for multiple platforms
3. Creates a GitHub release
4. Uploads binaries and checksums

## Supported Platforms

- **macOS**: Intel (x86_64) and Apple Silicon (aarch64/M1/M2/M3)
- **Linux**: x86_64 and ARM64 (aarch64)
- **Windows**: x86_64

## Creating a Release

### 1. Update Version

Update the version in `Cargo.toml`:

```toml
[workspace.package]
version = "0.2.0"  # Update this
```

### 2. Update CHANGELOG (Optional but Recommended)

Create/update `CHANGELOG.md` with release notes:

```markdown
## [0.2.0] - 2024-02-08

### Added
- Structured logging with tracing
- Metrics HTTP API on localhost:9090
- `devcli metrics` CLI command

### Changed
- Improved error messages
- Better health check logging

### Fixed
- Bug in process restart logic
```

### 3. Commit Changes

```bash
git add Cargo.toml CHANGELOG.md
git commit -m "chore: bump version to 0.2.0"
git push origin develop
```

### 4. Create and Push Tag

```bash
# Create annotated tag
git tag -a v0.2.0 -m "Release v0.2.0"

# Push tag to trigger release workflow
git push origin v0.2.0
```

### 5. Monitor Workflow

1. Go to GitHub Actions tab
2. Watch the "Release" workflow run
3. It will:
   - Build for all platforms (~10-15 minutes)
   - Create GitHub release
   - Upload binaries and checksums

### 6. Edit Release Notes (Optional)

1. Go to GitHub Releases page
2. Edit the auto-created release
3. Add detailed release notes, breaking changes, upgrade instructions

## Release Workflow Details

### Platforms Built

| Platform | Target Triple | Archive Format |
|----------|---------------|----------------|
| macOS (Intel) | x86_64-apple-darwin | .tar.gz |
| macOS (ARM) | aarch64-apple-darwin | .tar.gz |
| Linux (x64) | x86_64-unknown-linux-gnu | .tar.gz |
| Linux (ARM64) | aarch64-unknown-linux-gnu | .tar.gz |
| Windows (x64) | x86_64-pc-windows-msvc | .zip |

### Artifacts

Each release includes:

- **Binary archives**: `devcli-{target}.tar.gz` or `.zip`
- **Checksums**: `devcli-{target}.{tar.gz,zip}.sha256`
- **Installation instructions**: In release notes

### Verification

After release, verify:

```bash
# Download binary
curl -L https://github.com/YOUR_USERNAME/devcli/releases/download/v0.2.0/devcli-x86_64-apple-darwin.tar.gz | tar xz

# Verify checksum
curl -L https://github.com/YOUR_USERNAME/devcli/releases/download/v0.2.0/devcli-x86_64-apple-darwin.tar.gz.sha256
shasum -a 256 devcli-x86_64-apple-darwin.tar.gz

# Test binary
./devcli --version
./devcli --help
```

## Versioning

We follow [Semantic Versioning](https://semver.org/):

- **MAJOR** (1.0.0): Breaking changes
- **MINOR** (0.1.0): New features, backwards compatible
- **PATCH** (0.0.1): Bug fixes, backwards compatible

### Pre-releases

For alpha/beta releases, use tags like:

```bash
git tag -a v0.2.0-alpha.1 -m "Alpha release"
git tag -a v0.2.0-beta.1 -m "Beta release"
git tag -a v0.2.0-rc.1 -m "Release candidate"
```

These will still trigger the workflow but can be marked as "pre-release" on GitHub.

## Troubleshooting

### Workflow Fails

1. Check GitHub Actions logs for errors
2. Common issues:
   - Compilation errors (fix in code, delete tag, re-tag)
   - Missing permissions (check GITHUB_TOKEN)
   - Network issues (retry workflow)

### Delete and Re-release

If you need to redo a release:

```bash
# Delete local tag
git tag -d v0.2.0

# Delete remote tag
git push origin :refs/tags/v0.2.0

# Delete GitHub release (via web UI)

# Fix issues, then re-tag
git tag -a v0.2.0 -m "Release v0.2.0"
git push origin v0.2.0
```

### Test Release Workflow Locally

You can't fully test the release workflow locally (needs GitHub), but you can test builds:

```bash
# Build for your platform
cargo build --release

# Cross-compile for another platform (requires cross)
cargo install cross
cross build --release --target x86_64-unknown-linux-gnu
```

## CI Workflow

The CI workflow runs on every push and PR to `main` or `develop`:

- **Test job**: Runs tests on Ubuntu and macOS
- **Lint job**: Checks formatting and runs clippy
- **Build job**: Builds on Ubuntu, macOS, and Windows

This ensures code quality before releases.

## Future Enhancements

- [ ] Publish to crates.io automatically
- [ ] Create Homebrew formula
- [ ] Create Debian/RPM packages
- [ ] Sign binaries for macOS/Windows
- [ ] Notarize macOS binaries
- [ ] Create Docker images
- [ ] Automated changelog generation

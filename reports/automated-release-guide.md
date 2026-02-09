# Automated Release Workflow Guide

## Overview

The release workflow is now **fully automated** using **Release Please** (by Google). No more manual versioning, tagging, or changelog writing!

## How It Works

```
1. You commit to develop branch
   ↓
2. Release Please analyzes commits
   ↓
3. Automatically creates "Release PR"
   - Bumps version in Cargo.toml
   - Updates CHANGELOG.md
   - Ready for review
   ↓
4. You merge the Release PR
   ↓
5. Release Please creates tag (e.g., v0.2.0)
   ↓
6. Tag triggers release-public.yml workflow
   ↓
7. Builds binaries, syncs docs to public repo
   ↓
8. Release published! 🎉
```

## Commit Message Convention

Release Please uses **Conventional Commits** to determine version bumps.

### Format

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Types and Version Bumps

**Patch Version** (0.1.0 → 0.1.1):
```bash
fix: correct health check timeout
fix(monitor): prevent restart loop
perf: improve startup time
```

**Minor Version** (0.1.0 → 0.2.0):
```bash
feat: add TUI log viewer
feat(config): support YAML configuration
feat!: add metrics API endpoint
```

**Major Version** (0.1.0 → 1.0.0):
```bash
feat!: redesign configuration format

BREAKING CHANGE: Configuration now uses YAML instead of JSON
```

Or:
```bash
feat: add new health check system

BREAKING CHANGE: Old health check format no longer supported
```

### Other Types

These don't trigger releases but appear in changelog:

```bash
docs: update README installation steps
style: format code with rustfmt
refactor: simplify process spawning logic
test: add integration tests for monitor
build: update dependencies
ci: improve GitHub Actions caching
chore: update license information
```

## Workflow Steps

### 1. Make Changes and Commit

Work on your feature and commit with conventional commit messages:

```bash
git checkout -b feature/tui-improvements
# Make changes
git add .
git commit -m "feat(tui): add syntax highlighting to log viewer"
git push origin feature/tui-improvements
```

### 2. Merge to Develop

Create PR and merge to `develop` branch:

```bash
# Via GitHub PR, or:
git checkout develop
git merge feature/tui-improvements
git push origin develop
```

### 3. Release Please Creates PR

**Automatically** (within minutes), Release Please will:
- Analyze commits since last release
- Determine version bump (patch/minor/major)
- Update `Cargo.toml` with new version
- Generate `CHANGELOG.md` entries
- Create a "Release PR" titled like: `chore(main): release 0.2.0`

Example Release PR content:
```markdown
## 0.2.0 (2024-02-09)

### Features

* **tui**: add syntax highlighting to log viewer ([abc123](link))
* **config**: support YAML configuration ([def456](link))

### Bug Fixes

* **monitor**: prevent restart loop ([ghi789](link))
```

### 4. Review Release PR

GitHub will show you the Release PR. Review:
- ✅ Version bump is correct (patch/minor/major)
- ✅ CHANGELOG.md looks good
- ✅ Cargo.toml versions updated

### 5. Merge Release PR

When ready to release:

```bash
# Merge the Release PR via GitHub UI or CLI
gh pr merge <PR_NUMBER> --merge
```

**Immediately after merge:**
- Release Please creates the git tag (e.g., `v0.2.0`)
- Tag creation triggers `release-public.yml`
- Binaries built and uploaded
- Docs synced to public repo
- Release published!

### 6. Verify Release

Check that everything worked:

```bash
# Check private repo
git fetch --tags
git tag  # Should see v0.2.0

# Check public repo
gh release list --repo xDelph/devcli
# Should see v0.2.0 with binaries

# Check docs synced
curl https://github.com/xDelph/devcli/blob/main/CHANGELOG.md
```

## Configuration Files

### `.release-please-manifest.json`

Tracks current version:
```json
{
  ".": "0.1.0"
}
```

Release Please updates this automatically.

### `release-please-config.json`

Configures Release Please behavior:
- Release type: `rust`
- Package name: `devcli`
- Files to update: All Cargo.toml files
- Tag format: `v{version}`

### `.github/workflows/release-please.yml`

Runs on every push to `develop`:
- Creates/updates Release PR
- Creates tag when Release PR merged

### `.github/workflows/release-public.yml`

Triggers on tag creation:
- Builds binaries for all platforms
- Syncs docs to public repo
- Creates release in public repo

## Examples

### Scenario 1: Bug Fix Release

```bash
# Fix a bug
git checkout -b fix/health-check-timeout
# ... make fixes
git commit -m "fix(monitor): increase health check timeout to 30s"
git push

# Merge PR to develop
# Release Please creates PR: "chore(main): release 0.1.1"
# Merge Release PR → v0.1.1 released
```

**Version**: 0.1.0 → **0.1.1** (patch)

### Scenario 2: New Feature

```bash
# Add feature
git checkout -b feat/metrics-api
# ... implement feature
git commit -m "feat(metrics): add HTTP API for metrics collection"
git push

# Merge to develop
# Release Please creates PR: "chore(main): release 0.2.0"
# Merge Release PR → v0.2.0 released
```

**Version**: 0.1.1 → **0.2.0** (minor)

### Scenario 3: Breaking Change

```bash
# Breaking change
git checkout -b feat/yaml-config
# ... implement new config format
git commit -m "feat(config): migrate to YAML configuration

BREAKING CHANGE: Configuration files now use YAML format (.devcli/config.yml)
instead of JSON (.devcli/config.json). Run 'devcli config migrate' to upgrade."
git push

# Merge to develop
# Release Please creates PR: "chore(main): release 1.0.0"
# Merge Release PR → v1.0.0 released
```

**Version**: 0.2.0 → **1.0.0** (major)

### Scenario 4: Multiple Changes

```bash
# Multiple commits in one PR
git checkout -b multi-improvements
git commit -m "feat(tui): add color themes"
git commit -m "fix(process): handle SIGTERM gracefully"
git commit -m "perf(monitor): reduce polling overhead"
git commit -m "docs: update installation guide"
git push

# Merge to develop
# Release Please combines all commits
# Creates PR: "chore(main): release 0.3.0"
#
# CHANGELOG will include:
# ### Features
# - tui: add color themes
#
# ### Bug Fixes
# - process: handle SIGTERM gracefully
#
# ### Performance
# - monitor: reduce polling overhead
#
# ### Documentation
# - update installation guide
```

**Version**: 0.2.0 → **0.3.0** (minor, because of feat)

## CHANGELOG.md

Release Please automatically generates and maintains `CHANGELOG.md`:

```markdown
# Changelog

## [0.2.0](https://github.com/xDelph/devcli/compare/v0.1.0...v0.2.0) (2024-02-09)

### Features

* **metrics**: add HTTP API for metrics collection ([abc123](link))
* **tui**: add syntax highlighting ([def456](link))

### Bug Fixes

* **monitor**: prevent restart loop ([ghi789](link))

## [0.1.0](https://github.com/xDelph/devcli/releases/tag/v0.1.0) (2024-02-01)

Initial release
```

Synced to public repo on each release.

## Tips & Best Practices

### 1. Use Conventional Commits

**Good**:
```bash
git commit -m "feat(ui): add dark mode support"
git commit -m "fix: resolve memory leak in monitor"
git commit -m "perf(startup): lazy-load configuration"
```

**Bad**:
```bash
git commit -m "added feature"
git commit -m "fixed bug"
git commit -m "updates"
```

### 2. Write Clear Commit Messages

Include **why**, not just **what**:

```bash
# Good
git commit -m "fix(monitor): increase timeout to prevent false positives

Health checks were timing out for slow-starting services,
causing unnecessary restarts. Increased default timeout
from 5s to 30s."

# Okay
git commit -m "fix(monitor): increase health check timeout"

# Bad
git commit -m "fix timeout"
```

### 3. Use Scopes

Scopes help organize changelog:

```bash
feat(config): ...   # Configuration features
feat(tui): ...      # Terminal UI features
feat(monitor): ...  # Monitoring features
fix(process): ...   # Process management fixes
```

### 4. Breaking Changes

Always explain migration path:

```bash
git commit -m "feat!: redesign health check configuration

BREAKING CHANGE: Health checks now use 'checks' key instead of 'health'.
Update your config.yml:

Before:
  health:
    type: http

After:
  checks:
    - type: http

Run 'devcli config migrate' to auto-upgrade."
```

### 5. Combine Related Commits

Use PR strategy:

```bash
# Multiple commits in feature branch
git commit -m "feat(api): add endpoint structure"
git commit -m "feat(api): implement handlers"
git commit -m "feat(api): add tests"
git commit -m "docs(api): document new API"

# When merged, Release Please sees all commits
# and includes all in release notes
```

## Troubleshooting

### Release PR Not Created

**Problem**: Pushed to develop but no Release PR appeared

**Check**:
1. Are commits using conventional format?
   ```bash
   git log --oneline
   # Should see: "feat: ...", "fix: ...", etc.
   ```

2. Is Release Please workflow enabled?
   ```bash
   # Check GitHub Actions tab
   # Look for "Release Please" workflow runs
   ```

3. Are there any releasable commits since last release?
   ```bash
   # Only feat, fix, perf, revert trigger releases
   # docs, style, chore don't trigger releases
   ```

**Fix**: Add a conventional commit:
```bash
git commit --allow-empty -m "chore: trigger release"
git push
```

### Wrong Version Bump

**Problem**: Expected minor bump, got patch bump

**Cause**: Commit type determines bump:
- `fix:` → patch
- `feat:` → minor
- `BREAKING CHANGE:` → major

**Fix**: Use correct commit type or add `!`:
```bash
git commit -m "feat!: add new feature with breaking change"
```

### CHANGELOG Missing Entries

**Problem**: Some commits not in CHANGELOG

**Cause**: Hidden changelog types (style, chore, test, build, ci)

**Fix**: Use visible types (feat, fix, perf, docs, refactor) or update config.

### Multiple Release PRs

**Problem**: Two Release PRs open at once

**Cause**: Merged to develop while Release PR open

**Fix**: Merge existing Release PR first, or close and let Release Please recreate.

## Reference

### Commit Types

| Type | Version Bump | In Changelog | Purpose |
|------|--------------|--------------|---------|
| `feat:` | minor | ✅ Features | New features |
| `fix:` | patch | ✅ Bug Fixes | Bug fixes |
| `perf:` | patch | ✅ Performance | Performance improvements |
| `revert:` | patch | ✅ Reverts | Revert previous commit |
| `docs:` | - | ✅ Documentation | Documentation only |
| `refactor:` | - | ✅ Refactoring | Code refactoring |
| `style:` | - | ❌ | Code style (formatting) |
| `test:` | - | ❌ | Adding tests |
| `build:` | - | ❌ | Build system changes |
| `ci:` | - | ❌ | CI/CD changes |
| `chore:` | - | ❌ | Other changes |

**Note**: Adding `!` after type forces minor bump (or major if `BREAKING CHANGE:` in body)

### Version Bumping Rules

- `fix:` → Patch (0.1.0 → 0.1.1)
- `feat:` → Minor (0.1.0 → 0.2.0)
- `feat!:` or `BREAKING CHANGE:` → Major (0.1.0 → 1.0.0)

**Pre-1.0.0 behavior**:
- Breaking changes bump minor (0.1.0 → 0.2.0)
- After 1.0.0, breaking changes bump major (1.0.0 → 2.0.0)

### Useful Commands

```bash
# View commits since last release
git log v0.1.0..HEAD --oneline

# Check conventional commit format
git log --oneline | grep -E "^[a-z]+(\(.+\))?:"

# Trigger release workflow manually
gh workflow run release-please.yml

# View Release Please PR
gh pr list --label "autorelease: pending"

# Close and recreate Release PR
gh pr close <PR_NUMBER>
# Push new commit to trigger recreation
```

## Summary

**Old Workflow** (Manual):
1. ❌ Manually update version in Cargo.toml
2. ❌ Manually write CHANGELOG.md
3. ❌ Manually create git tag
4. ❌ Push tag to trigger release

**New Workflow** (Automated):
1. ✅ Write conventional commits
2. ✅ Merge to develop
3. ✅ Review and merge Release PR
4. ✅ **Done!** - Tag, release, binaries, docs all automatic

**Benefits**:
- 🚀 Faster releases
- 📝 Automatic changelog
- 🔢 Consistent versioning
- 📦 Always up-to-date public repo
- ✅ Less manual work
- 🎯 Clear release history

---

**Ready to release?** Just use conventional commits and merge to develop!

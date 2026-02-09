# README and Docs Sync to Public Repo

## Problem

Currently, the release workflow only copies binaries to the public repo. We also need to sync:
- README.md
- LICENSE
- LICENSE-COMMERCIAL.md
- docs/ folder
- CHANGELOG.md (if you create one)
- install.sh (once created)

## Solution

Update the release workflow to sync documentation files to the public repo on each release.

## Two Approaches

### Approach A: Sync on Release (Recommended)

Sync docs when you create a release (alongside binaries).

**Pros:**
- ✅ Documentation always matches the release
- ✅ Simple - part of existing release workflow
- ✅ Single workflow to maintain

**Cons:**
- ⚠️ Docs only updated when you release
- ⚠️ Can't update docs independently

### Approach B: Separate Docs Workflow

Separate workflow to sync docs on any commit to main.

**Pros:**
- ✅ Update docs without releasing
- ✅ Keep docs current between releases

**Cons:**
- ⚠️ More complex (two workflows)
- ⚠️ Docs might be ahead of released version

**Recommendation: Use Approach A** - simpler and docs match releases.

---

## Implementation: Approach A (Sync on Release)

### Step 1: Update Release Workflow

Add a new job to sync documentation files to the public repo.

**Update**: `.github/workflows/release-public.yml`

Add this job at the end (after `build-release`):

```yaml
  sync-documentation:
    name: Sync Documentation to Public Repo
    needs: create-release
    runs-on: ubuntu-latest

    steps:
      - name: Checkout private repo
        uses: actions/checkout@v4
        with:
          path: private

      - name: Checkout public repo
        uses: actions/checkout@v4
        with:
          repository: ${{ env.PUBLIC_REPO_OWNER }}/${{ env.PUBLIC_REPO_NAME }}
          token: ${{ secrets.PUBLIC_REPO_TOKEN }}
          path: public

      - name: Sync documentation files
        run: |
          # Copy documentation files from private to public repo
          echo "📄 Syncing documentation files..."

          # Core files
          cp private/README.md public/README.md
          cp private/LICENSE public/LICENSE
          cp private/LICENSE-COMMERCIAL.md public/LICENSE-COMMERCIAL.md

          # Docs folder (if exists)
          if [ -d "private/docs" ]; then
            echo "📁 Syncing docs/ folder..."
            rm -rf public/docs
            cp -r private/docs public/docs
          fi

          # Install script (if exists)
          if [ -f "private/install.sh" ]; then
            echo "🚀 Syncing install.sh..."
            cp private/install.sh public/install.sh
            chmod +x public/install.sh
          fi

          # CHANGELOG (if exists)
          if [ -f "private/CHANGELOG.md" ]; then
            echo "📝 Syncing CHANGELOG.md..."
            cp private/CHANGELOG.md public/CHANGELOG.md
          fi

          echo "✅ Documentation sync complete"

      - name: Show changes
        run: |
          cd public
          git status

      - name: Commit and push to public repo
        run: |
          cd public
          git config user.name "GitHub Actions"
          git config user.email "actions@github.com"
          git add .

          # Only commit if there are changes
          if git diff --staged --quiet; then
            echo "No documentation changes to commit"
          else
            git commit -m "docs: sync documentation for release ${{ github.ref_name }}"
            git push
            echo "✅ Documentation pushed to public repo"
          fi
```

### Step 2: Files to Sync

**Always sync:**
- ✅ `README.md` - Project overview and installation
- ✅ `LICENSE` - License text
- ✅ `LICENSE-COMMERCIAL.md` - Commercial licensing terms

**Sync if exists:**
- ✅ `docs/` - All documentation
- ✅ `install.sh` - Installation script
- ✅ `CHANGELOG.md` - Release notes

**Never sync (keep private):**
- ❌ Source code (`devcli/`, `devcli-core/`)
- ❌ `.github/workflows/` - CI/CD workflows
- ❌ `Cargo.toml`, `Cargo.lock` - Build config
- ❌ `target/` - Build artifacts
- ❌ `reports/` - Internal planning and implementation docs

### Step 3: Selective Docs Sync (Optional)

If you have docs that should stay private, create two docs folders:

```
private-repo/
├── docs/              # Public docs (synced to public repo)
│   ├── getting-started.md
│   ├── configuration-reference.md
│   └── ...
└── docs-private/      # Private docs (NOT synced)
    ├── development.md
    ├── architecture.md
    └── ...
```

Update the workflow to only sync `docs/`:

```yaml
- name: Sync documentation files
  run: |
    # Only sync public docs
    if [ -d "private/docs" ]; then
      cp -r private/docs public/docs
    fi
    # Don't sync docs-private/
```

### Step 4: README Customization for Public Repo

If you want different READMEs for private vs public repo:

**Create**: `README.public.md` in private repo

```markdown
# devcli

[![License: PolyForm](https://img.shields.io/badge/License-PolyForm%20Noncommercial-blue.svg)](LICENSE)

A powerful CLI for managing spawned processes...

## Installation

### Recommended: Install Script
```bash
curl -fsSL https://raw.githubusercontent.com/xDelph/devcli/main/install.sh | sh
```

[Rest of README for end users...]

## Support

- 📖 [Documentation](https://github.com/xDelph/devcli/tree/main/docs)
- 🐛 [Issue Tracker](https://github.com/xDelph/devcli/issues)
- 📧 Commercial Licensing: devcli@delalonde.dev

---

**Note**: This is a binary distribution. Source code is proprietary.
```

Update workflow to use public version:

```yaml
- name: Sync documentation files
  run: |
    # Use public-specific README if it exists
    if [ -f "private/README.public.md" ]; then
      cp private/README.public.md public/README.md
    else
      cp private/README.md public/README.md
    fi

    # ... rest of sync
```

---

## Implementation: Approach B (Separate Docs Workflow)

If you want to update docs independently of releases:

### Create Docs Sync Workflow

**New file**: `.github/workflows/sync-docs.yml`

```yaml
name: Sync Documentation to Public Repo

on:
  push:
    branches:
      - main
    paths:
      - 'README.md'
      - 'LICENSE'
      - 'LICENSE-COMMERCIAL.md'
      - 'docs/**'
      - 'install.sh'
      - 'CHANGELOG.md'

  workflow_dispatch: # Allow manual trigger

env:
  PUBLIC_REPO_OWNER: xDelph
  PUBLIC_REPO_NAME: devcli

jobs:
  sync-docs:
    name: Sync Documentation
    runs-on: ubuntu-latest

    steps:
      - name: Checkout private repo
        uses: actions/checkout@v4
        with:
          path: private

      - name: Checkout public repo
        uses: actions/checkout@v4
        with:
          repository: ${{ env.PUBLIC_REPO_OWNER }}/${{ env.PUBLIC_REPO_NAME }}
          token: ${{ secrets.PUBLIC_REPO_TOKEN }}
          path: public

      - name: Sync documentation files
        run: |
          echo "📄 Syncing documentation files..."

          # Core files
          cp private/README.md public/README.md
          cp private/LICENSE public/LICENSE
          cp private/LICENSE-COMMERCIAL.md public/LICENSE-COMMERCIAL.md

          # Docs folder
          if [ -d "private/docs" ]; then
            rm -rf public/docs
            cp -r private/docs public/docs
          fi

          # Install script
          if [ -f "private/install.sh" ]; then
            cp private/install.sh public/install.sh
            chmod +x public/install.sh
          fi

          # CHANGELOG
          if [ -f "private/CHANGELOG.md" ]; then
            cp private/CHANGELOG.md public/CHANGELOG.md
          fi

          echo "✅ Documentation sync complete"

      - name: Commit and push to public repo
        run: |
          cd public
          git config user.name "GitHub Actions"
          git config user.email "actions@github.com"
          git add .

          if git diff --staged --quiet; then
            echo "No documentation changes to commit"
          else
            COMMIT_MSG="docs: sync from private repo (${GITHUB_SHA::7})"
            git commit -m "$COMMIT_MSG"
            git push
            echo "✅ Documentation pushed to public repo"
          fi
```

**Pros of Separate Workflow:**
- Updates docs on every commit to main
- Can fix typos without releasing
- Docs always current

**Cons:**
- Docs might be ahead of latest release
- More workflow complexity

---

## Testing

### Test Locally

Before pushing, test the sync locally:

```bash
# Clone both repos
git clone git@github.com:xDelph/devcli-private.git private
git clone git@github.com:xDelph/devcli.git public

# Sync files
cp private/README.md public/README.md
cp private/LICENSE public/LICENSE
cp private/LICENSE-COMMERCIAL.md public/LICENSE-COMMERCIAL.md
cp -r private/docs public/docs

# Check what changed
cd public
git status
git diff

# If looks good, commit and push
git add .
git commit -m "docs: sync documentation"
git push
```

### Test Workflow

1. Create a test tag in private repo:
   ```bash
   git tag v0.0.1-test
   git push origin v0.0.1-test
   ```

2. Watch GitHub Actions run
3. Check public repo for synced files
4. Delete test tag and release if successful:
   ```bash
   git tag -d v0.0.1-test
   git push origin :refs/tags/v0.0.1-test
   gh release delete v0.0.1-test --repo xDelph/devcli -y
   ```

---

## Complete Updated Release Workflow

Here's the complete workflow with documentation sync:

```yaml
name: Release to Public Repo

on:
  push:
    tags:
      - "v*.*.*"

env:
  CARGO_TERM_COLOR: always
  PUBLIC_REPO_OWNER: xDelph
  PUBLIC_REPO_NAME: devcli

jobs:
  create-release:
    name: Create Release in Public Repo
    runs-on: ubuntu-latest
    outputs:
      version: ${{ steps.get_version.outputs.version }}

    steps:
      - name: Checkout private repo
        uses: actions/checkout@v4

      - name: Get version from tag
        id: get_version
        run: echo "version=${GITHUB_REF#refs/tags/v}" >> $GITHUB_OUTPUT

      - name: Create release in public repo
        env:
          GH_TOKEN: ${{ secrets.PUBLIC_REPO_TOKEN }}
        run: |
          gh release create ${{ github.ref_name }} \
            --repo ${{ env.PUBLIC_REPO_OWNER }}/${{ env.PUBLIC_REPO_NAME }} \
            --title "Release ${{ github.ref_name }}" \
            --notes "See [CHANGELOG.md](https://github.com/${{ env.PUBLIC_REPO_OWNER }}/${{ env.PUBLIC_REPO_NAME }}/blob/main/CHANGELOG.md) for details."

  build-release:
    name: Build ${{ matrix.target }}
    needs: create-release
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: macos-latest
            target: x86_64-apple-darwin
            use_cross: false
          - os: macos-latest
            target: aarch64-apple-darwin
            use_cross: false
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            use_cross: false
          - os: ubuntu-latest
            target: aarch64-unknown-linux-gnu
            use_cross: true

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Install cross
        if: matrix.use_cross
        run: cargo install cross --git https://github.com/cross-rs/cross

      - name: Build release binary
        run: |
          if [ "${{ matrix.use_cross }}" = "true" ]; then
            cross build --release --target ${{ matrix.target }}
          else
            cargo build --release --target ${{ matrix.target }}
          fi

      - name: Strip binary
        run: strip target/${{ matrix.target }}/release/devcli

      - name: Create archive
        run: |
          cd target/${{ matrix.target }}/release
          tar czf ../../../devcli-${{ matrix.target }}.tar.gz devcli
          cd -

      - name: Generate checksum
        run: |
          shasum -a 256 devcli-${{ matrix.target }}.tar.gz > devcli-${{ matrix.target }}.tar.gz.sha256

      - name: Upload binaries to public repo release
        env:
          GH_TOKEN: ${{ secrets.PUBLIC_REPO_TOKEN }}
        run: |
          gh release upload ${{ github.ref_name }} \
            devcli-${{ matrix.target }}.tar.gz \
            devcli-${{ matrix.target }}.tar.gz.sha256 \
            --repo ${{ env.PUBLIC_REPO_OWNER }}/${{ env.PUBLIC_REPO_NAME }} \
            --clobber

  sync-documentation:
    name: Sync Documentation to Public Repo
    needs: create-release
    runs-on: ubuntu-latest

    steps:
      - name: Checkout private repo
        uses: actions/checkout@v4
        with:
          path: private

      - name: Checkout public repo
        uses: actions/checkout@v4
        with:
          repository: ${{ env.PUBLIC_REPO_OWNER }}/${{ env.PUBLIC_REPO_NAME }}
          token: ${{ secrets.PUBLIC_REPO_TOKEN }}
          path: public

      - name: Sync documentation files
        run: |
          echo "📄 Syncing documentation files..."

          # Core files
          cp private/README.md public/README.md
          cp private/LICENSE public/LICENSE
          cp private/LICENSE-COMMERCIAL.md public/LICENSE-COMMERCIAL.md

          # Docs folder
          if [ -d "private/docs" ]; then
            echo "📁 Syncing docs/ folder..."
            rm -rf public/docs
            cp -r private/docs public/docs
          fi

          # Install script
          if [ -f "private/install.sh" ]; then
            echo "🚀 Syncing install.sh..."
            cp private/install.sh public/install.sh
            chmod +x public/install.sh
          fi

          # CHANGELOG
          if [ -f "private/CHANGELOG.md" ]; then
            echo "📝 Syncing CHANGELOG.md..."
            cp private/CHANGELOG.md public/CHANGELOG.md
          fi

          echo "✅ Documentation sync complete"

      - name: Commit and push to public repo
        run: |
          cd public
          git config user.name "GitHub Actions"
          git config user.email "actions@github.com"
          git add .

          if git diff --staged --quiet; then
            echo "No documentation changes to commit"
          else
            git commit -m "docs: sync documentation for release ${{ github.ref_name }}"
            git push
            echo "✅ Documentation pushed to public repo"
          fi
```

---

## Checklist

When switching to public repo:

**Setup:**
- [ ] Create public repo `xDelph/devcli`
- [ ] Add `PUBLIC_REPO_TOKEN` secret to private repo
- [ ] Update `.github/workflows/release-public.yml` with sync job

**On Each Release:**
- [ ] Tag release in private repo: `git tag v0.1.0 && git push --tags`
- [ ] GitHub Actions automatically:
  - ✅ Creates release in public repo
  - ✅ Builds and uploads binaries
  - ✅ Syncs README, LICENSE, docs
- [ ] Verify public repo has updated docs

**Public Repo Should Have:**
- ✅ README.md (installation, usage)
- ✅ LICENSE (PolyForm Noncommercial)
- ✅ LICENSE-COMMERCIAL.md (commercial terms)
- ✅ docs/ (all user documentation)
- ✅ install.sh (install script)
- ✅ CHANGELOG.md (release notes)
- ✅ Binaries in Releases

**Public Repo Should NOT Have:**
- ❌ Source code (devcli/, devcli-core/)
- ❌ Cargo.toml, Cargo.lock
- ❌ .github/workflows/ (CI/CD)
- ❌ Development docs
- ❌ Any private/internal documentation

---

## Summary

**Problem**: README and docs only in private repo
**Solution**: Add sync job to release workflow
**Result**: Every release automatically updates docs in public repo

**Recommended Approach**: Sync on release (Approach A)
- Simple, reliable
- Docs match releases
- Single workflow

This ensures users always have current documentation matching the released binaries!

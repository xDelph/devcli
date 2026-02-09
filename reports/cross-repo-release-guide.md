# Cross-Repository Release Guide

Publishing releases from a private repo to a public repo with automated GitHub Actions.

## Overview

**Architecture**:
- **Private Repo** (`devcli-private`): Source code, development, CI
- **Public Repo** (`devcli`): Releases only, binaries, documentation

**Flow**:
```
Private Repo (push tag)
    ↓
CI builds binaries
    ↓
CI creates release in Public Repo
    ↓
CI uploads binaries to Public Repo
```

## Setup (One-Time)

### 1. Create Public Repository

```bash
# On GitHub, create new public repository
# Name: devcli
# Description: Process management CLI for developers
# Do NOT initialize with README (we'll push docs separately)
```

### 2. Create Personal Access Token (PAT)

1. Go to GitHub → Settings → Developer settings → Personal access tokens → Tokens (classic)
2. Click "Generate new token (classic)"
3. Name: `DEVCLI_PUBLIC_REPO_TOKEN`
4. Expiration: No expiration (or 1 year)
5. Scopes: Select **only**:
   - ✅ `public_repo` (Access public repositories)
   - ✅ `write:packages` (optional, for GitHub Packages)
6. Click "Generate token"
7. **Copy the token** (you won't see it again!)

**Security Note**: This token only has access to public repos, not your private repos.

### 3. Add Token to Private Repo Secrets

1. Go to private repo → Settings → Secrets and variables → Actions
2. Click "New repository secret"
3. Name: `PUBLIC_REPO_TOKEN`
4. Value: Paste the PAT from step 2
5. Click "Add secret"

### 4. Configure Public Repo Name

In your private repo, create `.github/config/release.env`:

```bash
# Public repository for releases
PUBLIC_REPO_OWNER=your-github-username
PUBLIC_REPO_NAME=devcli
```

Or use repository variables:
1. Private repo → Settings → Secrets and variables → Actions → Variables
2. Add `PUBLIC_REPO_OWNER` = `your-github-username`
3. Add `PUBLIC_REPO_NAME` = `devcli`

## Modified Release Workflow

Replace `.github/workflows/release.yml` with this version:

```yaml
name: Release

on:
  push:
    tags:
      - 'v*.*.*'

env:
  CARGO_TERM_COLOR: always
  # Public repo info (change these to your values)
  PUBLIC_REPO_OWNER: your-github-username
  PUBLIC_REPO_NAME: devcli

jobs:
  create-release:
    name: Create Release in Public Repo
    runs-on: ubuntu-latest
    outputs:
      upload_url: ${{ steps.create_release.outputs.upload_url }}
      release_id: ${{ steps.create_release.outputs.id }}
      version: ${{ steps.get_version.outputs.version }}

    steps:
      - name: Checkout private repo
        uses: actions/checkout@v4

      - name: Get version from tag
        id: get_version
        run: echo "version=${GITHUB_REF#refs/tags/v}" >> $GITHUB_OUTPUT

      - name: Create release in public repo
        id: create_release
        env:
          GH_TOKEN: ${{ secrets.PUBLIC_REPO_TOKEN }}
        run: |
          # Create release in public repo using gh CLI
          gh release create ${{ github.ref_name }} \
            --repo ${{ env.PUBLIC_REPO_OWNER }}/${{ env.PUBLIC_REPO_NAME }} \
            --title "Release ${{ github.ref_name }}" \
            --notes "$(cat <<'EOF'
          ## Installation

          Download the appropriate binary for your platform and add it to your PATH.

          ### macOS (Apple Silicon M1/M2/M3)
          \`\`\`bash
          curl -L https://github.com/${{ env.PUBLIC_REPO_OWNER }}/${{ env.PUBLIC_REPO_NAME }}/releases/download/${{ github.ref_name }}/devcli-aarch64-apple-darwin.tar.gz | tar xz
          sudo mv devcli /usr/local/bin/
          \`\`\`

          ### macOS (Intel)
          \`\`\`bash
          curl -L https://github.com/${{ env.PUBLIC_REPO_OWNER }}/${{ env.PUBLIC_REPO_NAME }}/releases/download/${{ github.ref_name }}/devcli-x86_64-apple-darwin.tar.gz | tar xz
          sudo mv devcli /usr/local/bin/
          \`\`\`

          ### Linux (x64)
          \`\`\`bash
          curl -L https://github.com/${{ env.PUBLIC_REPO_OWNER }}/${{ env.PUBLIC_REPO_NAME }}/releases/download/${{ github.ref_name }}/devcli-x86_64-unknown-linux-gnu.tar.gz | tar xz
          sudo mv devcli /usr/local/bin/
          \`\`\`

          ### Linux (ARM64)
          \`\`\`bash
          curl -L https://github.com/${{ env.PUBLIC_REPO_OWNER }}/${{ env.PUBLIC_REPO_NAME }}/releases/download/${{ github.ref_name }}/devcli-aarch64-unknown-linux-gnu.tar.gz | tar xz
          sudo mv devcli /usr/local/bin/
          \`\`\`

          ## Checksums

          SHA256 checksums are provided for each binary.

          ## Documentation

          See https://github.com/${{ env.PUBLIC_REPO_OWNER }}/${{ env.PUBLIC_REPO_NAME }} for documentation.
          EOF
          )"

          # Get release upload URL
          RELEASE_INFO=$(gh release view ${{ github.ref_name }} \
            --repo ${{ env.PUBLIC_REPO_OWNER }}/${{ env.PUBLIC_REPO_NAME }} \
            --json id,uploadUrl)

          echo "id=$(echo $RELEASE_INFO | jq -r .id)" >> $GITHUB_OUTPUT
          echo "upload_url=$(echo $RELEASE_INFO | jq -r .uploadUrl)" >> $GITHUB_OUTPUT

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

      - name: Cache cargo registry
        uses: actions/cache@v4
        with:
          path: ~/.cargo/registry
          key: ${{ runner.os }}-${{ matrix.target }}-cargo-registry-${{ hashFiles('**/Cargo.lock') }}

      - name: Cache cargo index
        uses: actions/cache@v4
        with:
          path: ~/.cargo/git
          key: ${{ runner.os }}-${{ matrix.target }}-cargo-index-${{ hashFiles('**/Cargo.lock') }}

      - name: Cache cargo build
        uses: actions/cache@v4
        with:
          path: target
          key: ${{ runner.os }}-${{ matrix.target }}-cargo-build-target-${{ hashFiles('**/Cargo.lock') }}

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

      - name: Upload archive to public repo
        env:
          GH_TOKEN: ${{ secrets.PUBLIC_REPO_TOKEN }}
        run: |
          gh release upload ${{ github.ref_name }} \
            devcli-${{ matrix.target }}.tar.gz \
            devcli-${{ matrix.target }}.tar.gz.sha256 \
            --repo ${{ env.PUBLIC_REPO_OWNER }}/${{ env.PUBLIC_REPO_NAME }}
```

## Push Documentation to Public Repo

### Option 1: Manual Initial Setup

```bash
# Clone public repo
git clone https://github.com/your-username/devcli.git devcli-public
cd devcli-public

# Copy docs from private repo
cp -r ../devcli-private/docs .
cp ../devcli-private/README.md .
cp ../devcli-private/LICENSE .
cp ../devcli-private/CHANGELOG.md .

# Create .gitignore
cat > .gitignore << 'EOF'
# No source code in public repo
/devcli-core/
/devcli/
/target/
Cargo.toml
Cargo.lock
*.rs
EOF

# Commit and push
git add .
git commit -m "docs: initial documentation"
git push origin main
```

### Option 2: Automated Doc Sync (Optional)

Create `.github/workflows/sync-docs.yml` in **private repo**:

```yaml
name: Sync Docs to Public Repo

on:
  push:
    branches: [main, develop]
    paths:
      - 'docs/**'
      - 'README.md'
      - 'CHANGELOG.md'

jobs:
  sync-docs:
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

      - name: Sync documentation
        run: |
          # Copy docs
          rsync -av --delete private/docs/ public/docs/
          cp private/README.md public/
          cp private/CHANGELOG.md public/
          cp private/LICENSE public/ 2>/dev/null || true

      - name: Commit and push to public repo
        working-directory: public
        run: |
          git config user.name "GitHub Actions"
          git config user.email "actions@github.com"

          if [ -n "$(git status --porcelain)" ]; then
            git add .
            git commit -m "docs: sync from private repo"
            git push
          else
            echo "No changes to commit"
          fi
```

## Usage

### Creating a Release

```bash
# In private repo
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0

# GitHub Actions will:
# 1. Build binaries in private repo
# 2. Create release in public repo
# 3. Upload binaries to public repo
# 4. Users download from public repo
```

### What Users See

Users visit: `https://github.com/your-username/devcli`

They see:
- ✅ Documentation (README, docs/)
- ✅ Releases with binaries
- ✅ Installation instructions
- ❌ Source code (not visible)

### Updating Documentation

**Manual**:
```bash
# Update in private repo
vim docs/getting-started.md
git commit -am "docs: update guide"
git push

# Manually sync to public
cd devcli-public
cp ../devcli-private/docs/getting-started.md docs/
git commit -am "docs: update guide"
git push
```

**Automated** (if using sync-docs workflow):
```bash
# Update in private repo
vim docs/getting-started.md
git commit -am "docs: update guide"
git push

# Auto-syncs to public repo via GitHub Actions
```

## Security Considerations

### ✅ Safe

- PAT only has `public_repo` scope
- Can only write to public repos
- Cannot access private repos
- Binaries contain no source code

### ⚠️ Important

- **Don't commit secrets** to public repo
- **Review release notes** before tagging
- **Don't include sensitive info** in binaries (API keys, etc.)
- **Binaries are public** once released

### 🔒 Best Practices

1. **Use `.gitignore` in public repo** to prevent accidental source commits
2. **Rotate PAT annually** for security
3. **Monitor public repo** for unwanted changes
4. **Sign binaries** (future enhancement) for authenticity

## Troubleshooting

### PAT Issues

**Problem**: `gh: Resource not accessible by integration`

**Solution**:
- Verify PAT has `public_repo` scope
- Check PAT hasn't expired
- Ensure secret is named `PUBLIC_REPO_TOKEN`

### Upload Failures

**Problem**: Binaries fail to upload

**Solution**:
```bash
# Test gh CLI locally
export GH_TOKEN=your_pat
gh release list --repo your-username/devcli

# Verify release exists
gh release view v0.1.0 --repo your-username/devcli
```

### Sync Conflicts

**Problem**: Doc sync creates conflicts

**Solution**:
```bash
# In public repo
git fetch origin
git reset --hard origin/main
git push --force
```

## Alternative: GitHub Packages

You could also use GitHub Packages (Container Registry):

```yaml
- name: Build Docker image
  run: docker build -t devcli:${{ github.ref_name }} .

- name: Push to GitHub Packages
  run: |
    echo ${{ secrets.PUBLIC_REPO_TOKEN }} | docker login ghcr.io -u ${{ github.actor }} --password-stdin
    docker tag devcli:${{ github.ref_name }} ghcr.io/${{ env.PUBLIC_REPO_OWNER }}/devcli:${{ github.ref_name }}
    docker push ghcr.io/${{ env.PUBLIC_REPO_OWNER }}/devcli:${{ github.ref_name }}
```

## Summary

**Setup Steps**:
1. ✅ Create public repo
2. ✅ Create PAT with `public_repo` scope
3. ✅ Add PAT as `PUBLIC_REPO_TOKEN` secret in private repo
4. ✅ Update release workflow with cross-repo logic
5. ✅ Push initial docs to public repo
6. ✅ Tag in private repo → binaries appear in public repo

**User Experience**:
- Users visit public repo
- Download pre-built binaries
- Read documentation
- Never see source code
- You maintain code privately

**Zero Manual Intervention After Setup!** 🎉

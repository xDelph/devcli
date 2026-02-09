# Quick Setup: Public Releases from Private Repo

Follow these steps to set up cross-repository releases (one-time setup).

## Prerequisites

- ✅ Private GitHub repository with source code
- ✅ GitHub account with ability to create public repos
- ⏱️ Time needed: ~10 minutes

## Step 1: Create Public Repository (2 min)

1. Go to https://github.com/new
2. Repository name: `devcli`
3. Description: `Process management CLI for developers`
4. Visibility: **Public**
5. **Do NOT** initialize with README/LICENSE/gitignore
6. Click "Create repository"

**Result**: Empty public repo at `https://github.com/YOUR_USERNAME/devcli`

## Step 2: Create Personal Access Token (3 min)

1. Go to https://github.com/settings/tokens/new
2. Note: `devcli-public-releases`
3. Expiration: `No expiration` (or 1 year)
4. Scopes: Select **ONLY**:
   - ✅ `public_repo` (Access public repositories)
5. Click "Generate token"
6. **Copy the token** immediately (starts with `ghp_...`)

**⚠️ Important**: Save this token securely - you won't see it again!

## Step 3: Add Token to Private Repo (1 min)

1. Go to your **private** repo → Settings → Secrets and variables → Actions
2. Click "New repository secret"
3. Name: `PUBLIC_REPO_TOKEN`
4. Value: Paste the token from Step 2
5. Click "Add secret"

**Verify**: Secret appears in list as `PUBLIC_REPO_TOKEN`

## Step 4: Update Workflow Configuration (2 min)

Edit `.github/workflows/release-public.yml`:

```yaml
env:
  CARGO_TERM_COLOR: always
  # ⚠️ CHANGE THESE to your values
  PUBLIC_REPO_OWNER: YOUR_GITHUB_USERNAME  # ← Change this
  PUBLIC_REPO_NAME: devcli                 # ← Or change if different
```

Replace `YOUR_GITHUB_USERNAME` with your actual GitHub username.

**Example**:
```yaml
PUBLIC_REPO_OWNER: thomasdelalonde
PUBLIC_REPO_NAME: devcli
```

## Step 5: Rename/Disable Old Workflow (1 min)

```bash
# Rename old workflow to keep as backup
mv .github/workflows/release.yml .github/workflows/release.yml.backup

# Or delete it
rm .github/workflows/release.yml
```

## Step 6: Push Initial Documentation to Public Repo (5 min)

```bash
# Clone the public repo
git clone https://github.com/YOUR_USERNAME/devcli.git devcli-public
cd devcli-public

# Copy documentation from private repo
cp -r ../devcli-private/docs .
cp ../devcli-private/README.md .
cp ../devcli-private/LICENSE .
cp ../devcli-private/CHANGELOG.md .

# Create .gitignore to prevent source code commits
cat > .gitignore << 'EOF'
# Source code stays in private repo
/devcli-core/
/devcli/
/target/
Cargo.toml
Cargo.lock
*.rs
!docs/**/*.rs  # Allow code examples in docs

# Rust
*.o
*.so
*.dylib
*.rlib

# IDE
.idea/
.vscode/
*.swp
*.swo

# OS
.DS_Store
Thumbs.db
EOF

# Commit and push
git add .
git commit -m "docs: initial documentation"
git push origin main
```

## Step 7: Test the Setup (2 min)

```bash
# In private repo
cd ../devcli-private

# Create test tag (or use real version)
git tag -a v0.1.0-test -m "Test release"
git push origin v0.1.0-test

# Watch GitHub Actions
# Go to: Actions tab in private repo
# You should see "Release to Public Repo" workflow running

# After ~10-15 minutes, check public repo:
# https://github.com/YOUR_USERNAME/devcli/releases
# You should see v0.1.0-test with 4 binaries!
```

If successful, delete the test release and tag:

```bash
# Delete test tag
git tag -d v0.1.0-test
git push origin :refs/tags/v0.1.0-test

# Delete test release in public repo (via GitHub web UI)
```

## Verification Checklist

After setup, verify:

- [ ] Public repo exists at `https://github.com/YOUR_USERNAME/devcli`
- [ ] PAT created with `public_repo` scope
- [ ] `PUBLIC_REPO_TOKEN` secret added to private repo
- [ ] Workflow updated with correct `PUBLIC_REPO_OWNER` and `PUBLIC_REPO_NAME`
- [ ] Documentation pushed to public repo
- [ ] `.gitignore` in public repo prevents source commits
- [ ] Test tag creates release in public repo
- [ ] Binaries uploaded successfully

## Usage After Setup

### Create a Release

```bash
# 1. In private repo, update version
# Edit Cargo.toml: version = "0.1.0"

# 2. Update CHANGELOG.md

# 3. Commit changes
git add Cargo.toml CHANGELOG.md
git commit -m "chore: release v0.1.0"
git push origin develop

# 4. Create and push tag
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0

# 5. GitHub Actions automatically:
#    - Builds binaries in private repo
#    - Creates release in public repo
#    - Uploads binaries to public repo

# 6. Users download from public repo!
```

### Update Documentation

```bash
# In private repo
vim docs/getting-started.md
git commit -am "docs: update guide"
git push

# Then manually sync to public repo
cd ../devcli-public
cp ../devcli-private/docs/getting-started.md docs/
git commit -am "docs: update guide"
git push
```

Or set up automated sync (see cross-repo-release-guide.md).

## What Users Experience

Users visit: `https://github.com/YOUR_USERNAME/devcli`

They see:
- ✅ README with installation instructions
- ✅ Complete documentation in `docs/`
- ✅ Releases with downloadable binaries
- ✅ CHANGELOG with version history
- ❌ **No source code** (stays private!)

## Troubleshooting

### "Resource not accessible by integration"

**Cause**: PAT doesn't have correct permissions

**Fix**:
1. Verify PAT has `public_repo` scope
2. Regenerate token if needed
3. Update `PUBLIC_REPO_TOKEN` secret

### "Repository not found"

**Cause**: Incorrect repo name/owner in workflow

**Fix**:
1. Verify `PUBLIC_REPO_OWNER` and `PUBLIC_REPO_NAME` are correct
2. Check spelling and capitalization
3. Ensure public repo exists

### Binaries not uploading

**Cause**: Release created but upload fails

**Fix**:
1. Check if release was created in public repo
2. View workflow logs for errors
3. Verify `gh` CLI command syntax
4. Try re-running failed jobs

### Public repo shows source code

**Cause**: `.gitignore` not properly set up

**Fix**:
```bash
cd devcli-public
# Remove source files if accidentally committed
git rm -r devcli-core devcli Cargo.toml Cargo.lock
git commit -m "Remove source code"
git push

# Add proper .gitignore (see Step 6)
```

## Security Notes

✅ **Safe**:
- PAT only has `public_repo` scope
- Cannot access private repositories
- Binaries contain no source code
- Users get compiled binaries only

⚠️ **Important**:
- Keep PAT secure (don't commit it!)
- Review release notes before pushing tags
- Binaries are permanently public once released
- Consider binary signing for authenticity (future enhancement)

## Next Steps

After successful setup:

1. Create your first real release (v0.1.0)
2. Share public repo URL with users
3. Monitor downloads and issues
4. Update documentation as needed
5. Consider setting up automated doc sync

---

**Setup Complete!** 🎉

Your workflow:
1. Develop in **private** repo
2. Tag to release
3. Binaries appear in **public** repo
4. Users download from public repo
5. Source code stays private

**Zero manual steps after initial setup!**

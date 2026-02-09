# Install Script + Homebrew Setup Guide

## Overview

This guide covers setting up better installation methods for devcli:
1. **Install Script** - Cross-platform shell script (primary method)
2. **Homebrew Tap** - macOS package manager integration (optional, better UX)

Both will be set up in the **public repository** after you switch to it.

## Part 1: Install Script

### What It Does

Single command installation that:
- Detects user's platform (macOS Intel/ARM, Linux x64/ARM64)
- Downloads correct binary from GitHub Releases
- Installs to `~/.devcli/bin` (no sudo required)
- Adds to PATH automatically
- Verifies installation

### User Experience

```bash
# Install
curl -fsSL https://raw.githubusercontent.com/xDelph/devcli/main/install.sh | sh

# Output:
🚀 Installing devcli...
✓ Detected platform: macOS ARM64
✓ Downloading devcli v0.1.0...
✓ Installed to ~/.devcli/bin/devcli
✓ Added to PATH in ~/.zshrc
🎉 devcli installed successfully!

Run: devcli --version
```

### Install Script Template

**Create in public repo**: `install.sh`

```bash
#!/bin/sh
# devcli installer script
# Usage: curl -fsSL https://raw.githubusercontent.com/xDelph/devcli/main/install.sh | sh

set -e

# Configuration
REPO="xDelph/devcli"
INSTALL_DIR="${HOME}/.devcli/bin"
BINARY_NAME="devcli"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Helper functions
info() {
    printf "${BLUE}ℹ${NC} %s\n" "$1"
}

success() {
    printf "${GREEN}✓${NC} %s\n" "$1"
}

error() {
    printf "${RED}✗${NC} %s\n" "$1" >&2
    exit 1
}

warning() {
    printf "${YELLOW}⚠${NC} %s\n" "$1"
}

# Detect platform
detect_platform() {
    local os
    local arch

    # Detect OS
    case "$(uname -s)" in
        Darwin)
            os="apple-darwin"
            ;;
        Linux)
            os="unknown-linux-gnu"
            ;;
        *)
            error "Unsupported operating system: $(uname -s)"
            ;;
    esac

    # Detect architecture
    case "$(uname -m)" in
        x86_64)
            arch="x86_64"
            ;;
        arm64|aarch64)
            arch="aarch64"
            ;;
        *)
            error "Unsupported architecture: $(uname -m)"
            ;;
    esac

    echo "${arch}-${os}"
}

# Get latest release version
get_latest_version() {
    local version
    version=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

    if [ -z "$version" ]; then
        error "Failed to get latest version"
    fi

    echo "$version"
}

# Download and install
install_binary() {
    local platform="$1"
    local version="$2"
    local download_url="https://github.com/${REPO}/releases/download/${version}/${BINARY_NAME}-${platform}.tar.gz"
    local temp_dir

    info "Installing devcli ${version} for ${platform}..."

    # Create temp directory
    temp_dir=$(mktemp -d)
    trap 'rm -rf "$temp_dir"' EXIT

    # Download
    info "Downloading from ${download_url}..."
    if ! curl -fsSL "$download_url" -o "${temp_dir}/${BINARY_NAME}.tar.gz"; then
        error "Failed to download devcli"
    fi
    success "Downloaded"

    # Extract
    info "Extracting..."
    if ! tar -xzf "${temp_dir}/${BINARY_NAME}.tar.gz" -C "$temp_dir"; then
        error "Failed to extract archive"
    fi
    success "Extracted"

    # Create install directory
    mkdir -p "$INSTALL_DIR"

    # Install binary
    info "Installing to ${INSTALL_DIR}/${BINARY_NAME}..."
    if ! mv "${temp_dir}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"; then
        error "Failed to install binary"
    fi

    # Make executable
    chmod +x "${INSTALL_DIR}/${BINARY_NAME}"
    success "Installed to ${INSTALL_DIR}/${BINARY_NAME}"
}

# Add to PATH
setup_path() {
    local shell_rc
    local path_line="export PATH=\"\$HOME/.devcli/bin:\$PATH\""

    # Detect shell
    if [ -n "$BASH_VERSION" ]; then
        shell_rc="$HOME/.bashrc"
    elif [ -n "$ZSH_VERSION" ]; then
        shell_rc="$HOME/.zshrc"
    else
        # Try to detect from SHELL env var
        case "$SHELL" in
            */bash)
                shell_rc="$HOME/.bashrc"
                ;;
            */zsh)
                shell_rc="$HOME/.zshrc"
                ;;
            *)
                shell_rc="$HOME/.profile"
                ;;
        esac
    fi

    # Check if already in PATH
    if echo "$PATH" | grep -q "$INSTALL_DIR"; then
        success "Already in PATH"
        return 0
    fi

    # Check if already in shell config
    if [ -f "$shell_rc" ] && grep -q ".devcli/bin" "$shell_rc"; then
        success "Already configured in $shell_rc"
        info "Run: source $shell_rc"
        return 0
    fi

    # Add to shell config
    if [ -f "$shell_rc" ]; then
        echo "" >> "$shell_rc"
        echo "# Added by devcli installer" >> "$shell_rc"
        echo "$path_line" >> "$shell_rc"
        success "Added to PATH in $shell_rc"
        info "Run: source $shell_rc"
    else
        warning "Could not find shell config file"
        info "Add this to your shell config manually:"
        echo "  $path_line"
    fi
}

# Verify installation
verify_installation() {
    if [ -x "${INSTALL_DIR}/${BINARY_NAME}" ]; then
        success "Installation verified"

        # Try to run version command
        if command -v "$BINARY_NAME" >/dev/null 2>&1; then
            info "Current version: $(${BINARY_NAME} --version 2>/dev/null || echo 'Run: source ~/.zshrc (or your shell config)')"
        else
            warning "Binary installed but not in PATH yet"
            info "Run: export PATH=\"\$HOME/.devcli/bin:\$PATH\""
            info "Or restart your shell"
        fi
    else
        error "Installation verification failed"
    fi
}

# Main installation flow
main() {
    echo "🚀 Installing devcli..."
    echo ""

    # Detect platform
    local platform
    platform=$(detect_platform)
    success "Detected platform: $platform"

    # Get latest version
    local version
    version=$(get_latest_version)
    success "Latest version: $version"

    # Install binary
    install_binary "$platform" "$version"

    # Setup PATH
    echo ""
    info "Setting up PATH..."
    setup_path

    # Verify
    echo ""
    verify_installation

    # Success message
    echo ""
    echo "${GREEN}🎉 devcli installed successfully!${NC}"
    echo ""
    echo "Get started:"
    echo "  devcli --help"
    echo "  devcli --version"
    echo ""
    echo "Documentation: https://github.com/${REPO}"
    echo "License: PolyForm Noncommercial 1.0.0"
    echo "Commercial licensing: devcli@delalonde.dev"
}

# Run main
main
```

### Testing the Script Locally

Before publishing, test it:

```bash
# In public repo
chmod +x install.sh

# Test with local file
sh install.sh

# Test with curl (after pushing to GitHub)
curl -fsSL https://raw.githubusercontent.com/xDelph/devcli/main/install.sh | sh
```

### Update README.md

In the public repo README, update installation section:

```markdown
## Installation

### Recommended: Install Script

**macOS and Linux:**
```bash
curl -fsSL https://raw.githubusercontent.com/xDelph/devcli/main/install.sh | sh
```

This will:
- Detect your platform automatically
- Download the latest version
- Install to `~/.devcli/bin`
- Add to your PATH
- No sudo required

### Manual Installation

If you prefer manual installation, download the binary for your platform:

**macOS (Apple Silicon M1/M2/M3):**
```bash
curl -L https://github.com/xDelph/devcli/releases/latest/download/devcli-aarch64-apple-darwin.tar.gz | tar xz
chmod +x devcli
mv devcli ~/.devcli/bin/  # or /usr/local/bin with sudo
```

**macOS (Intel):**
```bash
curl -L https://github.com/xDelph/devcli/releases/latest/download/devcli-x86_64-apple-darwin.tar.gz | tar xz
chmod +x devcli
mv devcli ~/.devcli/bin/
```

**Linux (x64):**
```bash
curl -L https://github.com/xDelph/devcli/releases/latest/download/devcli-x86_64-unknown-linux-gnu.tar.gz | tar xz
chmod +x devcli
mv devcli ~/.devcli/bin/
```

**Linux (ARM64):**
```bash
curl -L https://github.com/xDelph/devcli/releases/latest/download/devcli-aarch64-unknown-linux-gnu.tar.gz | tar xz
chmod +x devcli
mv devcli ~/.devcli/bin/
```

### Verify Installation

```bash
devcli --version
devcli --help
```
```

---

## Part 2: Homebrew Tap (Optional)

### What It Does

Provides native macOS package manager installation:

```bash
brew tap xDelph/devcli
brew install devcli
```

### User Experience

```bash
# Install
$ brew tap xDelph/devcli
$ brew install devcli

==> Downloading https://github.com/xDelph/devcli/releases/download/v0.1.0/devcli-aarch64-apple-darwin.tar.gz
==> Installing devcli from xDelph/devcli
🍺  /opt/homebrew/Cellar/devcli/0.1.0: 2 files, 5.2MB

# Update
$ brew upgrade devcli

# Uninstall
$ brew uninstall devcli
```

### Setup Steps

#### 1. Create Homebrew Tap Repository

Create a **new public repository**: `homebrew-devcli`

```bash
# On GitHub, create: xDelph/homebrew-devcli
# Clone it locally
git clone https://github.com/xDelph/homebrew-devcli.git
cd homebrew-devcli
```

#### 2. Create Formula File

**File**: `Formula/devcli.rb`

```ruby
class Devcli < Formula
  desc "Powerful CLI for managing spawned processes with config-based management"
  homepage "https://github.com/xDelph/devcli"
  version "0.1.0"
  license "PolyForm-Noncommercial-1.0.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/xDelph/devcli/releases/download/v0.1.0/devcli-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256_FOR_ARM64_BINARY"
    else
      url "https://github.com/xDelph/devcli/releases/download/v0.1.0/devcli-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256_FOR_X86_64_BINARY"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/xDelph/devcli/releases/download/v0.1.0/devcli-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256_FOR_LINUX_ARM64_BINARY"
    else
      url "https://github.com/xDelph/devcli/releases/download/v0.1.0/devcli-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256_FOR_LINUX_X86_64_BINARY"
    end
  end

  def install
    bin.install "devcli"
  end

  test do
    system "#{bin}/devcli", "--version"
  end
end
```

#### 3. Get SHA256 Checksums

After creating a release, get checksums:

```bash
# Download each binary and compute SHA256
curl -L https://github.com/xDelph/devcli/releases/download/v0.1.0/devcli-aarch64-apple-darwin.tar.gz -o /tmp/devcli-arm64.tar.gz
shasum -a 256 /tmp/devcli-arm64.tar.gz

# Repeat for each platform
# Update the sha256 values in the formula
```

#### 4. Push to GitHub

```bash
git add Formula/devcli.rb
git commit -m "Add devcli formula v0.1.0"
git push origin main
```

#### 5. Test Installation

```bash
# Tap the repository
brew tap xDelph/devcli

# Install
brew install devcli

# Verify
devcli --version
```

### Automate Formula Updates

**Option A: Manual Update Script**

Create `update-formula.sh` in the tap repo:

```bash
#!/bin/bash
# Update Homebrew formula with new release

VERSION=$1
if [ -z "$VERSION" ]; then
    echo "Usage: ./update-formula.sh v0.1.0"
    exit 1
fi

# Download binaries and compute checksums
declare -A checksums

platforms=(
    "aarch64-apple-darwin"
    "x86_64-apple-darwin"
    "aarch64-unknown-linux-gnu"
    "x86_64-unknown-linux-gnu"
)

for platform in "${platforms[@]}"; do
    url="https://github.com/xDelph/devcli/releases/download/${VERSION}/devcli-${platform}.tar.gz"
    echo "Downloading $url..."
    curl -sL "$url" -o "/tmp/devcli-${platform}.tar.gz"
    checksum=$(shasum -a 256 "/tmp/devcli-${platform}.tar.gz" | awk '{print $1}')
    checksums[$platform]=$checksum
    echo "$platform: $checksum"
done

# Update formula file
cat > Formula/devcli.rb <<EOF
class Devcli < Formula
  desc "Powerful CLI for managing spawned processes with config-based management"
  homepage "https://github.com/xDelph/devcli"
  version "${VERSION#v}"
  license "PolyForm-Noncommercial-1.0.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/xDelph/devcli/releases/download/${VERSION}/devcli-aarch64-apple-darwin.tar.gz"
      sha256 "${checksums[aarch64-apple-darwin]}"
    else
      url "https://github.com/xDelph/devcli/releases/download/${VERSION}/devcli-x86_64-apple-darwin.tar.gz"
      sha256 "${checksums[x86_64-apple-darwin]}"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/xDelph/devcli/releases/download/${VERSION}/devcli-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "${checksums[aarch64-unknown-linux-gnu]}"
    else
      url "https://github.com/xDelph/devcli/releases/download/${VERSION}/devcli-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "${checksums[x86_64-unknown-linux-gnu]}"
    end
  end

  def install
    bin.install "devcli"
  end

  test do
    system "#{bin}/devcli", "--version"
  end
end
EOF

echo "Formula updated! Review and commit:"
echo "  git diff Formula/devcli.rb"
echo "  git add Formula/devcli.rb"
echo "  git commit -m 'Update devcli to $VERSION'"
echo "  git push"
```

**Option B: GitHub Actions Automation**

In the `homebrew-devcli` repo, create `.github/workflows/update-formula.yml`:

```yaml
name: Update Formula

on:
  repository_dispatch:
    types: [new-release]

jobs:
  update:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Update formula
        env:
          VERSION: ${{ github.event.client_payload.version }}
        run: |
          ./update-formula.sh $VERSION

      - name: Commit and push
        run: |
          git config user.name "GitHub Actions"
          git config user.email "actions@github.com"
          git add Formula/devcli.rb
          git commit -m "Update devcli to $VERSION"
          git push
```

Then trigger from main repo's release workflow:

```yaml
# In private repo's .github/workflows/release-public.yml
# Add at the end:
- name: Trigger Homebrew formula update
  run: |
    curl -X POST \
      -H "Accept: application/vnd.github+json" \
      -H "Authorization: Bearer ${{ secrets.HOMEBREW_TAP_TOKEN }}" \
      https://api.github.com/repos/xDelph/homebrew-devcli/dispatches \
      -d '{"event_type":"new-release","client_payload":{"version":"${{ github.ref_name }}"}}'
```

### Update README (with Homebrew)

```markdown
## Installation

### macOS: Homebrew (Recommended)

```bash
brew tap xDelph/devcli
brew install devcli
```

### Cross-platform: Install Script

```bash
curl -fsSL https://raw.githubusercontent.com/xDelph/devcli/main/install.sh | sh
```

### Manual Installation

[Keep existing manual instructions...]
```

---

## Summary Checklist

### In Public Repo (after you switch)

**Install Script:**
- [ ] Create `install.sh` in repo root
- [ ] Make executable: `chmod +x install.sh`
- [ ] Test locally
- [ ] Push to GitHub
- [ ] Test with curl: `curl -fsSL https://raw.githubusercontent.com/xDelph/devcli/main/install.sh | sh`
- [ ] Update README installation section

**Homebrew Tap (Optional):**
- [ ] Create new repo: `xDelph/homebrew-devcli`
- [ ] Create `Formula/devcli.rb`
- [ ] Compute SHA256 checksums for binaries
- [ ] Update formula with checksums
- [ ] Push to GitHub
- [ ] Test: `brew tap xDelph/devcli && brew install devcli`
- [ ] Create `update-formula.sh` script
- [ ] (Optional) Set up GitHub Actions auto-update

### Benefits

**Install Script:**
- ✅ Works on all platforms
- ✅ Single command installation
- ✅ No dependencies
- ✅ Automatic PATH setup
- ✅ Easy to maintain

**Homebrew:**
- ✅ Native macOS experience
- ✅ Automatic updates with `brew upgrade`
- ✅ Dependency management
- ✅ Uninstall support
- ✅ Trusted by developers

### Maintenance

**On each release:**

With install script only:
- Binaries uploaded to GitHub Releases
- Users run install script (auto-gets latest)

With Homebrew:
- Upload binaries to GitHub Releases
- Run `update-formula.sh v0.2.0` in tap repo
- Commit and push formula update

Or set up GitHub Actions to automate Homebrew updates!

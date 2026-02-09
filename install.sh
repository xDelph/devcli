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

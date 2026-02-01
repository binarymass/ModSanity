#!/bin/bash
# ModSanity Installation Script

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}ModSanity Installation Script${NC}"
echo "=============================="
echo ""

# Check if running as root
if [ "$EUID" -eq 0 ]; then
    echo -e "${RED}Error: Do not run this script as root${NC}"
    exit 1
fi

# Detect installation prefix
PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
SHARE_DIR="$PREFIX/share/modsanity"

echo "Installation directories:"
echo "  Binary:        $BIN_DIR"
echo "  Shared files:  $SHARE_DIR"
echo ""

# Check if binary exists
if [ ! -f "target/release/modsanity" ]; then
    echo -e "${RED}Error: Binary not found. Please run 'cargo build --release' first${NC}"
    exit 1
fi

# Create directories
echo "Creating directories..."
mkdir -p "$BIN_DIR"
mkdir -p "$SHARE_DIR"

# Install binary
echo "Installing binary..."
install -m 755 target/release/modsanity "$BIN_DIR/modsanity"

# Install documentation
echo "Installing documentation..."
install -m 644 README.md "$SHARE_DIR/README.md"
install -m 644 LICENSE "$SHARE_DIR/LICENSE"
install -m 644 CHANGELOG.md "$SHARE_DIR/CHANGELOG.md"

# Copy example files if they exist
if [ -d "examples" ]; then
    mkdir -p "$SHARE_DIR/examples"
    cp -r examples/* "$SHARE_DIR/examples/"
fi

# Check if bin directory is in PATH
if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
    echo ""
    echo -e "${YELLOW}Warning: $BIN_DIR is not in your PATH${NC}"
    echo "Add the following line to your ~/.bashrc or ~/.zshrc:"
    echo ""
    echo "    export PATH=\"\$PATH:$BIN_DIR\""
    echo ""
fi

echo ""
echo -e "${GREEN}Installation complete!${NC}"
echo ""
echo "Quick start:"
echo "  1. Run 'modsanity' to launch the TUI"
echo "  2. Press F4 to go to Settings"
echo "  3. Add your NexusMods API key"
echo "  4. Press 'g' to select a game"
echo ""
echo "For more information, see: $SHARE_DIR/README.md"

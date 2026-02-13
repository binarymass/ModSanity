#!/bin/bash
# ModSanity Installation Script

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Rollback tracking
INSTALLED_FILES=()
CREATED_DIRS=()

rollback() {
    echo ""
    echo -e "${YELLOW}Rolling back installation...${NC}"

    for file in "${INSTALLED_FILES[@]}"; do
        if [ -f "$file" ]; then
            rm -f "$file" && echo "  Removed: $file" || echo "  Failed to remove: $file"
        fi
    done

    # Remove dirs in reverse order so children are removed before parents
    for (( i=${#CREATED_DIRS[@]}-1; i>=0; i-- )); do
        dir="${CREATED_DIRS[$i]}"
        if [ -d "$dir" ] && [ -z "$(ls -A "$dir")" ]; then
            rmdir "$dir" && echo "  Removed dir: $dir" || echo "  Failed to remove dir: $dir"
        fi
    done

    echo -e "${YELLOW}Rollback complete.${NC}"
}

error_exit() {
    local message="$1"
    local code="${2:-1}"
    echo ""
    echo -e "${RED}Error: ${message} (exit code: ${code})${NC}"
    rollback
    exit "$code"
}

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
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Installation directories:"
echo "  Binary:        $BIN_DIR"
echo "  Shared files:  $SHARE_DIR"
echo ""

# Pre-flight checks

# Check if binary exists
if [ ! -f "$SCRIPT_DIR/modsanity" ]; then
    echo -e "${RED}Error: File not found. Expected 'modsanity' in the same directory as this install script${NC}"
    exit 2
fi

if [ ! -f "$SCRIPT_DIR/README.md" ]; then
    echo -e "${RED}Error: File not found. Expected 'README.md' in the same directory as this install script${NC}"
    exit 2
fi

if [ ! -f "$SCRIPT_DIR/LICENSE" ]; then
    echo -e "${RED}Error: File not found. Expected 'LICENSE' in the same directory as this install script${NC}"
    exit 2
fi

# Create directories
echo "Creating directories..."

if [ ! -d "$BIN_DIR" ]; then
    mkdir -p "$BIN_DIR" || error_exit "Failed to create directory: $BIN_DIR" $?
    CREATED_DIRS+=("$BIN_DIR")
fi

if [ ! -d "$SHARE_DIR" ]; then
    mkdir -p "$SHARE_DIR" || error_exit "Failed to create directory: $SHARE_DIR" $?
    CREATED_DIRS+=("$SHARE_DIR")
fi

# Install binary
echo "Installing binary..."
install -m 755 "$SCRIPT_DIR/modsanity" "$BIN_DIR/modsanity" || error_exit "Failed to install binary" $?
INSTALLED_FILES+=("$BIN_DIR/modsanity")

# Install documentation
echo "Installing documentation..."

install -m 644 "$SCRIPT_DIR/README.md" "$SHARE_DIR/README.md" || error_exit "Failed to install README.md" $?
INSTALLED_FILES+=("$SHARE_DIR/README.md")

install -m 644 "$SCRIPT_DIR/LICENSE" "$SHARE_DIR/LICENSE" || error_exit "Failed to install LICENSE" $?
INSTALLED_FILES+=("$SHARE_DIR/LICENSE")

# Copy example files if they exist
echo "Copying example files..."

if [ -d "$SCRIPT_DIR/examples" ]; then
    mkdir -p "$SHARE_DIR/examples" || error_exit "Failed to create examples directory" $?
    CREATED_DIRS+=("$SHARE_DIR/examples")
    cp -r "$SCRIPT_DIR/examples/." "$SHARE_DIR/examples/" || error_exit "Failed to copy example files" $?
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

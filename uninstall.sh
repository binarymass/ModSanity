#!/bin/bash
# ModSanity Uninstallation Script

#Needs to be updated for the files and folders created after the first start.

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}ModSanity Uninstallation Script${NC}"
echo "================================"
echo ""

# Check if running as root
if [ "$EUID" -eq 0 ]; then
    echo -e "${RED}Error: Do not run this script as root${NC}"
    exit 1
fi

PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
SHARE_DIR="$PREFIX/share/modsanity"

echo "Uninstalling from:"
echo "  Binary:        $BIN_DIR"
echo "  Shared files:  $SHARE_DIR"
echo ""

# Confirmation prompt
read -r -p "Are you sure you want to uninstall ModSanity? [y/N] " confirm
if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
    echo "Uninstall cancelled."
    exit 0
fi

echo ""

FAILED=0

remove_file() {
    local file="$1"
    if [ -f "$file" ]; then
        rm -f "$file" && echo "  Removed: $file" || { echo -e "  ${RED}Failed to remove: $file${NC}"; FAILED=1; }
    else
        echo -e "  ${YELLOW}Skipped (not found): $file${NC}"
    fi
}

remove_dir() {
    local dir="$1"
    if [ -d "$dir" ]; then
        rm -rf "$dir" && echo "  Removed dir: $dir" || { echo -e "  ${RED}Failed to remove dir: $dir${NC}"; FAILED=1; }
    else
        echo -e "  ${YELLOW}Skipped (not found): $dir${NC}"
    fi
}

# Remove binary
echo "Removing binary..."
remove_file "$BIN_DIR/modsanity"

# Remove documentation and share directory
echo "Removing shared files..."
remove_file "$SHARE_DIR/README.md"
remove_file "$SHARE_DIR/LICENSE"
remove_dir  "$SHARE_DIR/examples"

# Remove the share directory itself if empty
if [ -d "$SHARE_DIR" ] && [ -z "$(ls -A "$SHARE_DIR")" ]; then
    remove_dir "$SHARE_DIR"
elif [ -d "$SHARE_DIR" ]; then
    echo -e "  ${YELLOW}Skipped dir (not empty): $SHARE_DIR${NC}"
fi

echo ""

if [ "$FAILED" -eq 1 ]; then
    echo -e "${RED}Uninstallation completed with errors. Some files may need to be removed manually.${NC}"
    exit 1
else
    echo -e "${GREEN}Uninstallation complete!${NC}"
fi
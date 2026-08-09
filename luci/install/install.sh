#!/bin/bash

set -e

echo "========================================"
echo "Luci CLI Installer for macOS"
echo "========================================"
echo

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo "Please run this script with sudo."
    exit 1
fi

# Create installation directory
INSTALL_DIR="/usr/local/bin"
echo "Installing luci and lucid to $INSTALL_DIR..."

# Copy binaries
cp -f luci "$INSTALL_DIR/luci"
cp -f lucid "$INSTALL_DIR/lucid"

# Make executable
chmod +x "$INSTALL_DIR/luci"
chmod +x "$INSTALL_DIR/lucid"

echo
echo "========================================"
echo "Installation Complete!"
echo "========================================"
echo
echo "You can now use:"
echo "  luci --help"
echo "  lucid --help"
echo

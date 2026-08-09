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
echo "Installing luci to $INSTALL_DIR..."

# Copy binaries
cp -f luci "$INSTALL_DIR/luci"

# Make executable
chmod +x "$INSTALL_DIR/luci"

echo
echo "========================================"
echo "Installation Complete!"
echo "========================================"
echo
echo "You can now use:"
echo "  luci --help"
echo "  lucid --help"
echo

#!/bin/sh
set -e

# Builds the One ROM Studio application and dmg packages for Linux.
#
# Pre-requisites:
# - Rust:
#
# ```sh
#   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# ```

# Check we're running on Linux
if [ "$(uname -s)" != "Linux" ]; then
    echo "Error: This script must be run on Linux" >&2
    exit 1
fi

#
# Setup
#

# Install required packages
sudo apt update && sudo apt install -y libudev-dev libusb-1.0-0-dev gcc-aarch64-linux-gnu

# Also need aarch64 libudev-dev and libusb files
sudo dpkg --add-architecture arm64

# On Ubuntu 22.04 and earlier, arm64 packages need ports.ubuntu.com
if [ -f /etc/os-release ]; then
    . /etc/os-release
    if [ "$ID" = "ubuntu" ] && [ "$VERSION_ID" = "22.04" ]; then
        echo "Configuring ports.ubuntu.com for arm64 packages (Ubuntu 22.04)"
        sudo sed -i 's/^deb /deb [arch=amd64] /' /etc/apt/sources.list
        echo "deb [arch=arm64] http://ports.ubuntu.com/ubuntu-ports jammy main universe" | sudo tee -a /etc/apt/sources.list
        echo "deb [arch=arm64] http://ports.ubuntu.com/ubuntu-ports jammy-updates main universe" | sudo tee -a /etc/apt/sources.list
        echo "deb [arch=arm64] http://ports.ubuntu.com/ubuntu-ports jammy-security main universe" | sudo tee -a /etc/apt/sources.list
    fi
fi

sudo apt update && sudo apt install -y libudev-dev:arm64 libusb-1.0-0-dev:arm64

# Verify they actually installed
if ! dpkg -l | grep -q "libudev-dev.*arm64"; then
    echo "ERROR: libudev-dev:arm64 not installed" >&2
    exit 1
fi
echo "libudev-dev:arm64 is installed."

# Install the Rust targets
rustup target add x86_64-unknown-linux-gnu
rustup target add aarch64-unknown-linux-gnu

# Install cargo-packager if not already installed
# Forced to ensure it builds against glibc version on this system
cargo install cargo-packager --locked --force

#
# Clean previous builds
#

cargo clean --target x86_64-unknown-linux-gnu
cargo clean --target aarch64-unknown-linux-gnu
rm -fr dist/*.deb

#
# Intel (x86_64)
#

# Build One ROM Studio
PACKAGER_TARGET="x86_64-unknown-linux-gnu"
echo "Building for target: $PACKAGER_TARGET"
cargo build --release --target $PACKAGER_TARGET

# Package as a dmg
echo "Packaging dmg for target: $PACKAGER_TARGET"
cargo packager --release --target $PACKAGER_TARGET --formats deb

echo "Linux x86_64 build complete."

#
# ARM (aarch64)
#

# Build One ROM Studio
# Note: Requires setting PKG_CONFIG_SYSROOT_DIR and PKG_CONFIG_PATH to find
# the arm64 libudev-dev files
PACKAGER_TARGET="aarch64-unknown-linux-gnu"
export PKG_CONFIG_SYSROOT_DIR=/
export PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
echo "Building for target: $PACKAGER_TARGET"
cargo build --release --target $PACKAGER_TARGET

# Package as a dmg
echo "Packaging dmg for target: $PACKAGER_TARGET"
cargo packager --release --target $PACKAGER_TARGET --formats deb
echo "Linux ARM64 build complete."

#
# Inject deb scripts
#
echo "Injecting deb scripts into generated .deb files..."
/bin/bash scripts/inject-deb-scripts.sh dist

echo "Build complete."
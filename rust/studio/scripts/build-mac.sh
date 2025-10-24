#!/bin/sh
set -e

# Builds the One ROM Studio application and dmg packages for macOS.
#
# Pre-requisites:
# - Rust:
#
# ```sh
#   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# ```
#
# - Homebrew:
#
# ```sh
#   /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
# ```
#
# - Python 3 and pip: https://www.python.org/downloads/macos/

# Check we're running on macOS
if [ "$(uname -s)" != "Darwin" ]; then
    echo "Error: This script must be run on macOS" >&2
    exit 1
fi

#
# Setup
#

# Set libusb to static linking
export LIBUSB_STATIC=1

# Install the Rust targets
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin

# Install cargo-packager if not already installed
cargo install cargo-packager --locked

# Install fileicon if not already installed
brew install fileicon

# Install python pip packages
python3 -m pip install --break-system-packages -r scripts/requirements.txt

#
# Clean previous builds
#

cargo clean --target x86_64-apple-darwin
cargo clean --target aarch64-apple-darwin
rm -fr dist/*.dmg

#
# Intel silicon (x86_64)
#

# Build One ROM Studio
PACKAGER_TARGET="x86_64-apple-darwin"
echo "Building for target: $PACKAGER_TARGET"
cargo build --release --target $PACKAGER_TARGET

# Package as a dmg
echo "Packaging dmg for target: $PACKAGER_TARGET"
cargo packager --release --target $PACKAGER_TARGET --formats dmg

# Update the dmg background, icon positions and volume icon
scripts/update-dmg.py

# Clean intermediate dmg and Cargo build
cargo clean --target x86_64-apple-darwin
rm dist/*_cargo.dmg

echo "Intel dmg build complete."

#
# Apple Silicon (aarch64)
#

# Build One ROM Studio
PACKAGER_TARGET="aarch64-apple-darwin"
echo "Building for target: $PACKAGER_TARGET"
cargo build --release --target $PACKAGER_TARGET

# Package as a dmg
echo "Packaging dmg for target: $PACKAGER_TARGET"
cargo packager --release --target $PACKAGER_TARGET --formats dmg

# Update the dmg background, icon positions and volume icon
scripts/update-dmg.py

# Clean intermediate dmg and Cargo build
cargo clean --target aarch64-apple-darwin
rm dist/*_cargo.dmg

echo "Apple silicon dmg build complete."
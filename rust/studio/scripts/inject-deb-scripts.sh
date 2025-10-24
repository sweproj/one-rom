#!/bin/bash

# This script modifies the .deb creates by cargo packager to include the postinst and postrm
# scripts for setting up udev rules.  (Cargo packager appears not to support this directly.)

set -e

TARGET_DIR="${1:-.}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Find all .deb files
DEB_FILES=$(find "$TARGET_DIR" -name "*.deb" -type f)

if [ -z "$DEB_FILES" ]; then
    echo "Error: No .deb files found in $TARGET_DIR"
    exit 1
fi

# Process each .deb file
while IFS= read -r DEB_FILE; do
    # Convert to absolute path
    DEB_FILE=$(realpath "$DEB_FILE")
    
    echo "Processing: $DEB_FILE"
    
    # Create temp directory for this deb
    TEMP_DIR=$(mktemp -d)
    
    cd "$TEMP_DIR"
    
    # Extract the deb
    ar x "$DEB_FILE"
    
    # Extract control archive
    mkdir control
    tar xzf control.tar.gz -C control
    
    # Copy maintainer scripts
    cp "$SCRIPT_DIR/postinst" control/
    cp "$SCRIPT_DIR/postrm" control/
    chmod 755 control/postinst
    chmod 755 control/postrm
    
    # Repackage control archive
    tar czf control.tar.gz -C control .
    
    # Rebuild the deb
    ar rcs "$DEB_FILE" debian-binary control.tar.gz data.tar.gz
    
    # Clean up this temp dir
    cd - > /dev/null
    rm -rf "$TEMP_DIR"
    
    echo "Successfully injected maintainer scripts into $DEB_FILE"
done <<< "$DEB_FILES"
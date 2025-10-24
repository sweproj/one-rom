#!/usr/bin/env python3
"""
Build macOS DMG with proper layout using cargo packager + dmgbuild.

This script assumes cargo packager has already run and produced a DMG.
It then:
1. Finds and mounts the cargo packager DMG
2. Extracts the .app bundle from that DMG
3. Archives the cargo packager DMG
4. Creates a multi-resolution ICNS file from PNG icons
5. Uses dmgbuild to create final DMG with proper layout, background, and icon positions
6. Sets custom icon on the DMG using fileicon

Dependencies:
  pip install dmgbuild toml
  brew install fileicon

macOS tools used (built-in):
  - hdiutil (mounting/unmounting DMGs)
  - iconutil (creating ICNS files)
"""

import subprocess
import sys
import os
import shutil
import tempfile
import plistlib
from pathlib import Path

try:
    import toml
except ImportError:
    print("Error: toml not installed. Run: pip install toml", file=sys.stderr)
    sys.exit(1)

try:
    import dmgbuild
except ImportError:
    print("Error: dmgbuild not installed. Run: pip install dmgbuild", file=sys.stderr)
    sys.exit(1)


# Configuration constants
BINARY_NAME = "onerom-studio"
PRODUCT_NAME = "One ROM Studio"
VOLUME_NAME = "One ROM Studio"
CARGO_TOML_PATH = "Cargo.toml"
DIST_DIR = "dist"

# Binary paths to check
BINARY_PATHS = [
    "../target/x86_64-apple-darwin/release/onerom-studio",
    "../target/aarch64-apple-darwin/release/onerom-studio"
]

# DMG settings (matching cargo packager config)
DMG_BACKGROUND = "assets/onerom-dmg.png"
DMG_WINDOW_X = 200
DMG_WINDOW_Y = 200
DMG_WINDOW_WIDTH = 540
DMG_WINDOW_HEIGHT = 380
DMG_APP_ICON_X = 140
DMG_APP_ICON_Y = 160
DMG_APPLICATIONS_ICON_X = 400
DMG_APPLICATIONS_ICON_Y = 160
DMG_ICON_SIZE = 128
DMG_TEXT_SIZE = 16
DMG_FORMAT = "UDBZ"  # Compressed

# License
LICENSE_FILE = "LICENSE.txt"
LICENSE_LANGUAGE = "en_US"

# Icon files for creating ICNS
# Maps size in pixels to file path
# These will be used to create a proper multi-resolution ICNS file
ICON_FILES = {
    16: "assets/onerom-16x16.png",
    32: "assets/onerom-32x32.png",
    64: "assets/onerom-64x64.png",
    128: "assets/onerom-128x128.png",
    256: "assets/onerom-256x256.png",
    512: "assets/onerom-512x512.png",
}

# Temp directory prefix
TEMP_DIR_PREFIX = "onerom_build_"

# Archive suffix for cargo packager DMG
CARGO_DMG_SUFFIX = "_cargo"


def parse_cargo_version():
    """Extract version from Cargo.toml."""
    try:
        with open(CARGO_TOML_PATH, 'r') as f:
            cargo_data = toml.load(f)
        version = cargo_data['package']['version']
        return version
    except Exception as e:
        print(f"Error parsing {CARGO_TOML_PATH}: {e}", file=sys.stderr)
        sys.exit(1)


def detect_architecture(binary_path):
    """
    Detect architecture from the binary path.
    Expected paths like: ../target/x86_64-apple-darwin/release/onerom-studio
                         ../target/aarch64-apple-darwin/release/onerom-studio
    """
    path_str = str(binary_path)
    
    if 'x86_64-apple-darwin' in path_str:
        return 'x64'
    elif 'aarch64-apple-darwin' in path_str:
        return 'aarch64'
    else:
        # Fallback: try to detect from file command
        try:
            result = subprocess.run(['file', binary_path], 
                                  capture_output=True, text=True, check=True)
            if 'x86_64' in result.stdout:
                return 'x64'
            elif 'aarch64' in result.stdout:
                return 'aarch64'
        except subprocess.CalledProcessError:
            pass
    
    print(f"Error: Cannot detect architecture from binary path: {binary_path}", file=sys.stderr)
    sys.exit(1)


def mount_dmg(dmg_path):
    """
    Mount a DMG and return the mount point.
    Uses -noverify -nobrowse for headless operation.
    Auto-accepts license agreement if present.
    """
    print(f"Mounting {dmg_path}...")
    try:
        result = subprocess.run(
            ['hdiutil', 'attach', dmg_path, '-noverify', '-nobrowse', '-noautoopen', '-plist'],
            input='y\n',
            capture_output=True, text=True, check=True
        )
        
        # Parse plist output to get mount point
        # The output may contain license text before the XML, so extract just the XML portion
        stdout = result.stdout
        
        # Find the start of the XML plist
        xml_start = stdout.find('<?xml')
        if xml_start == -1:
            print(f"Error: No XML plist found in hdiutil output:", file=sys.stderr)
            print(f"  hdiutil stdout:", file=sys.stderr)
            print(stdout, file=sys.stderr)
            sys.exit(1)
        
        # Extract just the XML portion
        xml_content = stdout[xml_start:]
        
        try:
            plist_data = plistlib.loads(xml_content.encode())
        except Exception as e:
            print(f"Error: Failed to parse hdiutil plist output:", file=sys.stderr)
            print(f"  Exception: {e}", file=sys.stderr)
            print(f"  XML content:", file=sys.stderr)
            print(xml_content, file=sys.stderr)
            sys.exit(1)
        
        # Find the mount point from system-entities
        for entity in plist_data.get('system-entities', []):
            if 'mount-point' in entity:
                mount_point = entity['mount-point']
                print(f"✓ Mounted at: {mount_point}")
                return mount_point
        
        print("Error: Could not find mount point in hdiutil output", file=sys.stderr)
        print(f"  hdiutil plist data: {plist_data}", file=sys.stderr)
        sys.exit(1)
        
    except subprocess.CalledProcessError as e:
        print(f"Error: hdiutil failed to mount DMG:", file=sys.stderr)
        print(f"  Return code: {e.returncode}", file=sys.stderr)
        print(f"  stdout: {e.stdout}", file=sys.stderr)
        print(f"  stderr: {e.stderr}", file=sys.stderr)
        sys.exit(1)


def unmount_dmg(mount_point):
    """Unmount a DMG."""
    print(f"Unmounting {mount_point}...")
    try:
        subprocess.run(['hdiutil', 'detach', mount_point], check=True)
        print("✓ Unmounted")
    except subprocess.CalledProcessError as e:
        print(f"Warning: Failed to unmount {mount_point}: {e}", file=sys.stderr)


def extract_app_bundle(mount_point, dest_dir):
    """
    Extract the .app bundle from mounted DMG.
    Returns the path to the extracted .app.
    """
    # Find the .app bundle in the mount point
    mount_path = Path(mount_point)
    app_bundles = list(mount_path.glob('*.app'))
    
    if not app_bundles:
        print(f"Error: No .app bundle found in {mount_point}", file=sys.stderr)
        sys.exit(1)
    
    if len(app_bundles) > 1:
        print(f"Warning: Multiple .app bundles found, using first: {app_bundles[0]}")
    
    app_bundle = app_bundles[0]
    dest_path = Path(dest_dir) / app_bundle.name
    
    print(f"Copying {app_bundle.name} to {dest_dir}...")
    shutil.copytree(app_bundle, dest_path)
    print(f"✓ Extracted to {dest_path}")
    
    return dest_path


def build_dmg_with_dmgbuild(app_bundle_path, output_dmg, version, arch):
    """Create final DMG using dmgbuild with proper layout."""
    print(f"Building final DMG with dmgbuild...")
    
    app_name = app_bundle_path.name
    
    # dmgbuild settings matching cargo packager config
    settings = {
        'format': DMG_FORMAT,
        'size': None,      # Auto-calculate
        
        # Files to include
        'files': [str(app_bundle_path)],
        
        # Create Applications symlink
        'symlinks': {'Applications': '/Applications'},
        
        # Icon positions (x, y from top-left)
        'icon_locations': {
            app_name: (DMG_APP_ICON_X, DMG_APP_ICON_Y),
            'Applications': (DMG_APPLICATIONS_ICON_X, DMG_APPLICATIONS_ICON_Y)
        },
        
        # Background image
        'background': DMG_BACKGROUND,
        
        # Window settings
        'window_rect': ((DMG_WINDOW_X, DMG_WINDOW_Y), (DMG_WINDOW_WIDTH, DMG_WINDOW_HEIGHT)),
        
        # Icon and text size
        'icon_size': DMG_ICON_SIZE,
        'text_size': DMG_TEXT_SIZE,
        
        # License agreement
        'license': {
            'default-language': LICENSE_LANGUAGE,
            'licenses': {LICENSE_LANGUAGE: LICENSE_FILE}
        }
    }
    
    # Build the DMG
    dmgbuild.build_dmg(output_dmg, VOLUME_NAME, settings=settings)
    print(f"✓ Created {output_dmg}")


def create_icns_from_pngs(output_icns_path):
    """
    Create a proper ICNS file from the PNG icon files.
    Uses iconutil which is built into macOS and works headlessly.
    Returns the path to the created ICNS file.
    
    Exits with error if any required icon files are missing or ICNS creation fails.
    """
    print(f"Creating ICNS file from PNG icons...")
    
    # Verify all required icon files exist
    missing_icons = []
    for size, path in ICON_FILES.items():
        if not Path(path).exists():
            missing_icons.append(f"{size}x{size}: {path}")
    
    if missing_icons:
        print(f"Error: Required icon files are missing:", file=sys.stderr)
        for missing in missing_icons:
            print(f"  {missing}", file=sys.stderr)
        sys.exit(1)
    
    # Create a temporary iconset directory
    iconset_dir = tempfile.mkdtemp(suffix=".iconset")
    
    try:
        # Mapping of iconset filenames to source sizes
        # Format: (iconset_filename, source_size)
        iconset_mapping = [
            ('icon_16x16.png', 16),
            ('icon_16x16@2x.png', 32),
            ('icon_32x32.png', 32),
            ('icon_32x32@2x.png', 64),
            ('icon_128x128.png', 128),
            ('icon_128x128@2x.png', 256),
            ('icon_256x256.png', 256),
            ('icon_256x256@2x.png', 512),
            ('icon_512x512.png', 512),
            ('icon_512x512@2x.png', 1024),  # If available
        ]
        
        # Copy icons into iconset directory
        for iconset_filename, source_size in iconset_mapping:
            if source_size not in ICON_FILES:
                # Skip sizes we don't have (e.g., 1024x1024)
                continue
            
            source_path = ICON_FILES[source_size]
            dest_path = Path(iconset_dir) / iconset_filename
            
            shutil.copy(source_path, dest_path)
        
        # Convert iconset to ICNS using iconutil
        try:
            subprocess.run(
                ['iconutil', '-c', 'icns', iconset_dir, '-o', output_icns_path],
                check=True,
                capture_output=True,
                text=True
            )
        except subprocess.CalledProcessError as e:
            print(f"Error: iconutil failed to create ICNS:", file=sys.stderr)
            print(f"  stdout: {e.stdout}", file=sys.stderr)
            print(f"  stderr: {e.stderr}", file=sys.stderr)
            sys.exit(1)
        
        if not Path(output_icns_path).exists():
            print(f"Error: iconutil did not create ICNS file at {output_icns_path}", file=sys.stderr)
            sys.exit(1)
        
        print(f"✓ Created ICNS: {output_icns_path}")
        return output_icns_path
        
    finally:
        # Clean up temporary iconset directory
        if os.path.exists(iconset_dir):
            shutil.rmtree(iconset_dir)


def set_dmg_icon(dmg_path, icns_path):
    """
    Set a custom icon on the DMG file using fileicon.
    
    Exits with error if fileicon is not available or fails to set the icon.
    """
    print(f"Setting custom icon on DMG...")
    
    # Check if fileicon is available
    try:
        subprocess.run(
            ['which', 'fileicon'],
            check=True,
            capture_output=True
        )
    except subprocess.CalledProcessError:
        print(f"Error: fileicon not found. Install with: brew install fileicon", file=sys.stderr)
        sys.exit(1)
    
    # Set the icon
    try:
        subprocess.run(
            ['fileicon', 'set', str(dmg_path), str(icns_path)],
            check=True,
            capture_output=True,
            text=True
        )
    except subprocess.CalledProcessError as e:
        print(f"Error: fileicon failed to set icon on DMG:", file=sys.stderr)
        print(f"  stdout: {e.stdout}", file=sys.stderr)
        print(f"  stderr: {e.stderr}", file=sys.stderr)
        sys.exit(1)
    
    print(f"✓ Set custom icon on DMG")


def main():
    """Main build process."""
    # Find binary path
    binary_path = None
    for path in BINARY_PATHS:
        p = Path(path)
        if p.exists():
            binary_path = p
            break
    
    if not binary_path:
        print(f"Error: Binary not found at any of these paths:", file=sys.stderr)
        for path in BINARY_PATHS:
            print(f"  {path}", file=sys.stderr)
        sys.exit(1)
    
    # Parse version and architecture
    version = parse_cargo_version()
    arch = detect_architecture(binary_path)
    
    print(f"Building {PRODUCT_NAME} v{version} for {arch}")
    
    # Output paths
    dist_dir = Path(DIST_DIR)
    dist_dir.mkdir(exist_ok=True)
    
    final_dmg_name = f"{PRODUCT_NAME}_{version}_{arch}.dmg"
    final_dmg_path = dist_dir / final_dmg_name
    cargo_dmg_archived_name = f"{PRODUCT_NAME}_{version}_{arch}{CARGO_DMG_SUFFIX}.dmg"
    cargo_dmg_archived_path = dist_dir / cargo_dmg_archived_name
    
    # Create a temporary directory for extracted .app
    temp_app_dir = tempfile.mkdtemp(prefix=TEMP_DIR_PREFIX)
    icns_path = dist_dir / "temp_icon.icns"
    mount_point = None
    cargo_packager_dmg = None
    
    try:
        # Step 1: Find the cargo packager DMG
        # cargo packager creates: dist/One ROM Studio_0.1.0_x64.dmg
        cargo_packager_dmg = final_dmg_path
        
        if not cargo_packager_dmg.exists():
            print(f"Error: Cargo packager DMG not found at {cargo_packager_dmg}", file=sys.stderr)
            sys.exit(1)
        
        print(f"Found cargo packager DMG: {cargo_packager_dmg}")
        
        # Step 2: Mount the cargo packager DMG
        mount_point = mount_dmg(cargo_packager_dmg)
        
        # Step 3: Extract .app bundle
        app_bundle_path = extract_app_bundle(mount_point, temp_app_dir)
        
        # Step 4: Unmount the cargo packager DMG
        unmount_dmg(mount_point)
        mount_point = None
        
        # Step 5: Rename cargo packager DMG for archival
        print(f"Archiving cargo packager DMG as {cargo_dmg_archived_name}...")
        shutil.move(str(cargo_packager_dmg), str(cargo_dmg_archived_path))
        
        # Step 6: Create ICNS file from PNG icons
        create_icns_from_pngs(icns_path)
        
        # Step 7: Build final DMG with dmgbuild
        build_dmg_with_dmgbuild(app_bundle_path, final_dmg_path, version, arch)
        
        # Step 8: Set custom icon on DMG using fileicon
        set_dmg_icon(final_dmg_path, icns_path)
        
        print(f"\n✓ Successfully created: {final_dmg_path}")
        
    except Exception as e:
        print(f"\n✗ Build failed: {e}", file=sys.stderr)
        sys.exit(1)
    
    finally:
        # Cleanup
        if mount_point:
            unmount_dmg(mount_point)
        
        if os.path.exists(temp_app_dir):
            print(f"Cleaning up temporary app directory...")
            shutil.rmtree(temp_app_dir)
        
        if icns_path.exists():
            print(f"Cleaning up temporary ICNS file...")
            icns_path.unlink()


if __name__ == '__main__':
    main()
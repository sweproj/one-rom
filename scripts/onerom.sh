#!/usr/bin/env bash

# Script to build One ROM firmware and flash it to device using a JSON chip
# config file.

set -e

usage() {
    echo "Usage: $0 [-d] [-l] [-f] <board> [<mcu>] <onerom-config.json>"
    echo "  -d: Enable debug logging (requires -l)"
    echo "  -l: Enable boot logging"
    echo "  -f: Flash firmware to device"
    echo "  -v: Verbose output"
}

help() {
    echo "This script builds One ROM firmware for a specific hardware"
    echo "variant, appends metadata based on the config provided, and"
    echo "flashes it to the device."
    echo ""
    echo "For Fire boards (containing 'fire' in name), MCU defaults to"
    echo "rp2350. For Ice boards, MCU is required (e.g., f411re)."
}

if [ $# -lt 1 ]; then
    usage
    exit 1
fi

if [ "$1" == "--help" ] || [ "$1" == "-h" ]; then
    usage
    echo ""
    help
    exit 0
fi

# Parse the optional flags
BOOT_LOGGING=0
DEBUG_LOGGING=0
FLASH=0
VERBOSE=0
while getopts "dlfv" opt; do
  case $opt in
    d)
      DEBUG_LOGGING=1
      ;;
    l)
      BOOT_LOGGING=1
      ;;
    f)
      FLASH=1
      ;;
    v)
      VERBOSE=1
      ;;
    \?)
      echo "Invalid option: -$OPTARG" >&2
      usage
      exit 1
      ;;
  esac
done

# Check that -d requires -l
if [ $DEBUG_LOGGING -eq 1 ] && [ $BOOT_LOGGING -eq 0 ]; then
    echo "Error: -d requires -l" >&2
    usage
    exit 1
fi

# Shift the parsed options away
shift $((OPTIND - 1))

# Determine arguments based on count
BOARD=""
MCU=""
ONEROM_CONFIG=""

if [ $# -eq 2 ]; then
    # 2 args: board config
    BOARD=$1
    ONEROM_CONFIG=$2
    # Check if board contains "fire"
    if [[ ! "$BOARD" =~ [Ff]ire ]]; then
        echo "Error: MCU required for Ice boards" >&2
        usage
        exit 1
    fi
    MCU="rp2350"
elif [ $# -eq 3 ]; then
    # 3 args: board mcu config
    BOARD=$1
    MCU=$2
    ONEROM_CONFIG=$3
else
    usage
    exit 1
fi

# Determine board type from MCU and expand MCU for build if needed
BOARD_TYPE=""
MCU_FOR_BUILD=""
MCU_FOR_PATH=""
if [[ "$MCU" == "rp2350" ]]; then
    BOARD_TYPE="fire"
    MCU_FOR_BUILD="rp2350"
    MCU_FOR_PATH="$MCU"
else
    # Ice board - prepend stm32 for the build
    BOARD_TYPE="ice"
    MCU_FOR_BUILD="$MCU" 
    MCU_FOR_PATH="stm32${MCU}"
fi

# Extract config name from ONEROM_CONFIG path
CONFIG_NAME=$(basename "$ONEROM_CONFIG" .json)

# Construct output base name
OUTPUT_DIR="builds/fw"
mkdir -p "$OUTPUT_DIR"

if [ "$BOARD_TYPE" == "fire" ]; then
    OUTPUT_BASE="${OUTPUT_DIR}/onerom_${BOARD}_${CONFIG_NAME}"
else
    # Ice board - use short MCU form in filename
    OUTPUT_BASE="${OUTPUT_DIR}/onerom_${BOARD}_${MCU}_${CONFIG_NAME}"
fi

echo "One ROM Build Configuration:"
echo "- BOARD=$BOARD"
echo "- MCU=$MCU"
echo "- ONEROM_CONFIG=$ONEROM_CONFIG"
if [ $BOOT_LOGGING -eq 1 ]; then
    echo "- BOOT_LOGGING=enabled"
else
    echo "- BOOT_LOGGING=disabled"
fi
if [ $DEBUG_LOGGING -eq 1 ]; then
    echo "- DEBUG_LOGGING=enabled"
else
    echo "- DEBUG_LOGGING=disabled"
fi
if [ $VERBOSE -eq 1 ]; then
    echo "- BOARD_TYPE=$BOARD_TYPE"
    echo "- OUTPUT_BASE=$OUTPUT_BASE"
    echo "- MCU_FOR_BUILD=$MCU_FOR_BUILD"
    echo "- MCU_FOR_PATH=$MCU_FOR_PATH"
fi

# Build the firmware
echo "---"
echo "Building firmware..."
BUILD_FLAGS=""
[ $BOOT_LOGGING -eq 1 ] && BUILD_FLAGS="$BUILD_FLAGS -l"
[ $DEBUG_LOGGING -eq 1 ] && BUILD_FLAGS="$BUILD_FLAGS -d"
if [ $VERBOSE -eq 1 ]; then
    scripts/build-empty-fw.sh $BUILD_FLAGS "$BOARD" "$MCU_FOR_BUILD"
else
    scripts/build-empty-fw.sh $BUILD_FLAGS "$BOARD" "$MCU_FOR_BUILD" >/dev/null
fi

# Input base is the build output (named after expanded MCU)
INPUT_BASE="sdrr/build/sdrr-${MCU_FOR_PATH}"

# Add metadata
echo "---"
echo "Adding metadata..."
if [ $VERBOSE -eq 1 ]; then
    scripts/add.sh "$INPUT_BASE" "$OUTPUT_BASE" "$ONEROM_CONFIG"
else
    scripts/add.sh "$INPUT_BASE" "$OUTPUT_BASE" "$ONEROM_CONFIG" >/dev/null
fi

# Create UF2 or DFU if appropriate tool is available
if [ "$BOARD_TYPE" == "fire" ]; then
    if command -v picotool >/dev/null 2>&1; then
        echo "Creating UF2 file..."
        if [ $VERBOSE -eq 1 ]; then
            picotool uf2 convert "${OUTPUT_BASE}.bin" "${OUTPUT_BASE}.uf2"
        else
            picotool uf2 convert "${OUTPUT_BASE}.bin" "${OUTPUT_BASE}.uf2" >/dev/null
        fi
    else
        if [ $VERBOSE -eq 1 ]; then
            echo "picotool not found; skipping UF2 creation."
        fi
    fi
else
    # Ice board
    if command -v dfu-suffix >/dev/null 2>&1; then
        echo "Creating DFU file..."
        cp "${OUTPUT_BASE}.bin" "${OUTPUT_BASE}.dfu"
        if [ $VERBOSE -eq 1 ]; then
            dfu-suffix -v 0x0483 -p 0xdf11 -a "${OUTPUT_BASE}.dfu"
            echo "DFU binary created: ${OUTPUT_BASE}.dfu"
        else
            dfu-suffix -v 0x0483 -p 0xdf11 -a "${OUTPUT_BASE}.dfu" >/dev/null
        fi
    else
        if [ $VERBOSE -eq 1 ]; then
            echo "dfu-suffix tool not found; skipping DFU creation."
        fi
    fi
fi

# List created files
echo "---"
echo "Created files:"
ls -l "${OUTPUT_BASE}".* 2>/dev/null || true

# Flash if requested
if [ $FLASH -eq 1 ]; then
    echo "---"
    echo "Flashing firmware..."
    # Determine chip for probe-rs
    if [ "$BOARD_TYPE" == "fire" ]; then
        CHIP="RP235X"
    else
        # Ice board - convert to probe-rs format
        CHIP=$(echo "$MCU_FOR_PATH" | tr '[:lower:]' '[:upper:]')
        CHIP="${CHIP}TX"
    fi
    scripts/_flash-and-run-elf.sh "${OUTPUT_BASE}.elf" "$CHIP"
else
    echo "---"
    echo "Done"
fi
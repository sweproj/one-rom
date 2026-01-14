#!/usr/bin/env bash

# Used to create a stock, empty firmware image for One ROM, for a specific
# hardware variant

set -e

usage() {
    echo "Usage: $0 [-d] [-l] <board> <mcu>"
}

help() {
    echo "This script creates a stock, empty firmware image for"
    echo "One ROM, for a specific hardware variant."
}

if [ $# -lt 1 ]; then
    usage
    exit 1
fi

if [ $1 == "--help" ] || [ $1 == "-h" ]; then
    usage
    echo ""
    help
    exit 0
fi

if [ $# -lt 2 ]; then
    usage
    exit 1
fi

# Parse the optional flags
BOOT_LOGGING=0
DEBUG_LOGGING=0
while getopts "dl" opt; do
  case $opt in
    d)
      DEBUG_LOGGING=1
      ;;
    l)
      BOOT_LOGGING=1
      ;;
    \?)
      echo "Invalid option: -$OPTARG" >&2
      usage
      exit 1
      ;;
  esac
done

# Now shift the parsed options away to get to the positional arguments
shift $((OPTIND -1))
BOARD=$1
MCU=$2

echo "BOOT_LOGGING=$BOOT_LOGGING DEBUG_LOGGING=$DEBUG_LOGGING EXCLUDE_METADATA=1 ROM_CONFIGS= HW_REV=$BOARD MCU=$MCU make"
BOOT_LOGGING=$BOOT_LOGGING DEBUG_LOGGING=$DEBUG_LOGGING EXCLUDE_METADATA=1 ROM_CONFIGS= HW_REV=$BOARD MCU=$MCU make

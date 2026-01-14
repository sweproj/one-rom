#!/usr/bin/env bash

# Used to flash a One ROM binary firmware created by onerom-fw, and attach
# (using SWD/RTT) to it on the command line.

set -e

usage() {
    echo "Usage: $0 <firmware.elf> <chip>"
}

help() {
    echo "This script flashes an ELF to the One ROM device using"
    echo "probe-rs, and attaches to it for RTT output."
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

FIRMWARE="$1"
CHIP="$2"

echo "probe-rs run --chip "$CHIP" "$FIRMWARE""
probe-rs run --chip "$CHIP" "$FIRMWARE"
#!/usr/bin/env bash

# Script to append metadata to a One ROM firmware binary using onerom-fw,
# and run it using probe-rs.

set -e

usage() {
    echo "Usage: $0 <input_base> <output_base> <rom-config.json>"
}

help() {
    echo "This script appends metadata to a One ROM firmware binary"
    echo "using onerom-fw, then converts it to an ELF file with the"
    echo "correct _SEGGER_RTT symbol for debugging, flashes it to"
    echo "the One ROM device using probe-rs, and attaches to it for"
    echo "RTT output."
    echo ""
    echo "Params:"
    echo "  <input_base>      Base name of input binary/elf files (without extension)"
    echo "  <output_base>     Base name of output binary/elf files (without extension)"
    echo "  <rom-config.json> Path to ROM configuration JSON file"
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

if [ $# -lt 3 ]; then
    usage
    exit 1
fi

INPUT_BASE=$1
OUTPUT_BASE=$2
ROM_CONFIG=$3

INPUT_BIN="${INPUT_BASE}.bin"
INPUT_ELF="${INPUT_BASE}.elf"
OUTPUT_BIN="${OUTPUT_BASE}.bin"
OUTPUT_ELF="${OUTPUT_BASE}.elf"

# Construct chip ID from INPUT_BASE
BASE=$(basename "$INPUT_BASE")
if [[ "$BASE" == sdrr-stm32f* ]]; then
    CHIP=$(echo "$BASE" | sed 's/sdrr-//' | tr '[:lower:]' '[:upper:]')
    CHIP="${CHIP}TX"
elif [[ "$BASE" == sdrr-rp2350* ]]; then
    CHIP="RP235X"
else
    echo "Error: Could not determine chip from input base name '$BASE'"
    exit 1
fi

scripts/_append-metadata.sh "${INPUT_BIN}" "${OUTPUT_BIN}" "${ROM_CONFIG}"
scripts/_bin-to-elf.sh "${INPUT_ELF}" "${OUTPUT_BIN}" "${OUTPUT_ELF}" "${CHIP}"

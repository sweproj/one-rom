#!/usr/bin/env bash

# Script used to create a One ROM firmware binary from an ELF firmware file
# with no metadata, using onerom-fw and the provided ROM configurationß.

set -e

usage() {
    echo "Usage: $0 <input.bin> <output.bin> <rom-config.json>"
}

help() {
    echo "This script creates a One ROM binary firmware from an ELF"
    echo "firmware file with no metadata, using onerom-fw and the"
    echo "provided ROM configuration."
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

INPUT_BIN="$1"
OUTPUT_BIN="$2"
ROM_CONFIG="$3"

cargo run --manifest-path rust/Cargo.toml \
  --release \
  --bin onerom-fw -- \
  --fw-image "${INPUT_BIN}" \
  --out "${OUTPUT_BIN}" \
  --json "${ROM_CONFIG}"

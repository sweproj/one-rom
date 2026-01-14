#!/usr/bin/env bash

# Script used by bin-to-elf.sh to convert a modified binary firmware back into
# an ELF file with the correct _SEGGER_RTT symbol for debugging.

set -e

usage() {
    echo "Usage: $0 <original.elf> <modified.bin> <output.elf> <chip>"
}

help() {
    echo "This script converts a binary firmware created by onerom-fw"
    echo "into an ELF file with the correct _SEGGER_RTT symbol for"
    echo "debugging."
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

if [ $# -lt 4 ]; then
    usage
    exit 1
fi

ORIGINAL_ELF=$1
MODIFIED_BIN=$2
OUTPUT_ELF=$3
CHIP=$4

# If CHIP starts with STM32, use 0x08000000, else use 0x10000000
if [[ "$CHIP" == STM32* ]]; then
    FLASH_ADDR=0x08000000
else
    FLASH_ADDR=0x10000000
fi

RTT_ADDR=$(${TOOLCHAIN}/arm-none-eabi-nm "${ORIGINAL_ELF}" | grep ' _SEGGER_RTT$' | awk '{print "0x" $1}')

if [ -z "${RTT_ADDR}" ]; then
    echo "Error: Could not find _SEGGER_RTT symbol in ${ORIGINAL_ELF}"
    exit 1
fi

TEMP_ELF=$(mktemp)

${TOOLCHAIN}/arm-none-eabi-ld -Ttext=${FLASH_ADDR} -b binary \
  -e ${FLASH_ADDR} \
  "${MODIFIED_BIN}" -o "${TEMP_ELF}"

${TOOLCHAIN}/arm-none-eabi-objcopy --add-symbol _SEGGER_RTT=${RTT_ADDR} \
  "${TEMP_ELF}" "${OUTPUT_ELF}"

rm "${TEMP_ELF}"

echo "Created ${OUTPUT_ELF} with _SEGGER_RTT at ${RTT_ADDR}"

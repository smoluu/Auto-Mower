#!/bin/bash

OUTPUT_DIR="ota-binary"
mkdir -p "$OUTPUT_DIR"
TARGET_ELF="target/xtensa-esp32s3-espidf/release/turf_terminator_5000"

if [ ! -f "$TARGET_ELF" ]; then
    echo "Error: ELF file not found at $TARGET_ELF"
    echo "Ensure you have run 'cargo build --release --target xtensa-esp32s3-none-elf' first."
    exit 1
fi

esptool.py --chip esp32s3 elf2image "$TARGET_ELF"
if [ $? -ne 0 ]; then
    echo "Error: esptool.py elf2image failed"
    exit 1
fi

GENERATED_BIN="$(dirname "$TARGET_ELF")/turf_terminator_5000.bin"
if [ ! -f "$GENERATED_BIN" ]; then
    echo "Error: Generated .bin not found at $GENERATED_BIN"
    exit 1
fi

OUTPUT_BIN="$OUTPUT_DIR/firmware.bin"
cp "$GENERATED_BIN" "$OUTPUT_BIN"
if [ $? -ne 0 ]; then
    echo "Error: Failed to copy firmware binary to $OUTPUT_BIN"
    exit 1
fi

echo "OTA binary created at $OUTPUT_BIN"
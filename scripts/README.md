# Scripts

Helper scripts for testing and debugging.

## add-and-run.sh

Tkes a blank firmware image, adds the ROM metadata to it, flashes it and connects to it.

Example:

```bash
./add-and-run.sh sdrr/build/sdrr-rp2350.bin /tmp/onerom-fw rom-config/c64.json
```

Creates:

- `/tmp/onerom-fw.bin` - the firmware image with the ROM metadata added
- `/tmp/onerom-fw.elf` - the ELF file with symbols for debugging

Then flashes `/tmp/onerom-fw.elf` to the device and connects to it via `probe-rs`.



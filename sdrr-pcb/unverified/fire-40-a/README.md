# One ROM Fire 40 pin 

**Unverified** - This an experimental design to emulate a 27C400 16-bit 40-pin ROM.  As of its creation, there is no firmware support for this board.

## Contents

- [Schematic](./fire-40-a-schematic.pdf)
- [Fab Files](fab/)
- [KiCad Design Files](kicad/)
- [Errata](#errata)
- [Notes](#notes)
- [Changelog](#changelog)
- [BOM](#bom)

## Errata

## Notes

The fab files assume this is fabbed with both an RP2354B (i.e. containing 2MB flash on-board) _and_ external 2MB flash, with the appropriate R11/R12 configurations.  To have other variants assembled:
- RP2354 without external flash, do not populate U5 or R12
- RP2350 with external flash, do not populate R12, instead populate R11 with 0R resistor, U5 populated.

The RP2354B variant is recommended, if RP2354B parts are available, as it is cheaper than RP2350B + external flash.  External flash is also useful, to increase the number of ROM images that can be stored and selected at boot time, from 3 (internal flash only) to 7 (with external flash).

This board requires both top and bottom assembly, as many passives are located on the bottom side.  This requires JLC's standard PCB assembly (with higher costs as a result).  If the boards are fabbed individually (not panelised), JLB will require additional side rails to be added to bring the board up to their minimum PCB size for assembly (roughly 70mm x 70mm).  JLC will recommend this automatically during the ordering process.

As ever, take care that every parts is positioned correctly before ordering.  In particular note the pink pin one dot is located in the appropriate corner for each IC, and that diodes are oriented correctly.

## Changelog

## BOM

See fab files.

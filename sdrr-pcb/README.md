# sdrr-pcb

Contains SDRR PCB design files.  These are organised into two directories:

- [`verified/`](verified/README.md) - Verified designs that have been tested and work.
- [`unverified/`](unverified/README.md) - Unverified designs that have not been tested.

## Recommended Revisions

Last Updated 2026-01-01.

### Fire (RP2350)

The current recommended 24 pin hardware revision is [fire-24-c](./verified/fire-24-c/README.md).  This is a combined Pro (SWD) and USB version, with 2 image select jumpers.  It supports PIO and CPU serving algorithms.  There are 2 image select jumpers.  This is recommended over revisions A (Pro only) and B (USB only).

The current recommended 28 pin hardware revision is [fire-28-a2](./verified/fire-28-a2/README.md).  This is a combined Pro (SWD) and USB version, with 2 image select pins.  (The 28 pin version does not have X pins.)  It supports PIO and CPU serving algorithms.  Note that SWD pins CLK/DIO are incorrectly silkscreened in this revision - CLK is DIO and vice versa.  If this labelling is a concern, use currently unverified revision A3 [fire-28-a3](./unverified/fire-28-a3/README.md), which corrects this silkscreen error.

### Ice (STM32F4)

The current recommended 24 pin hardware revision is [ice-24-i](./verified/ice-24-i/README.md).  This is a combined Pro (SWD) and USB version, with 2 image select pins and 2 X (special function) pins.  It supports both SWD programming and USB programming.  Future firmware versions may extend from 2 to 4 image select pins without the need for additional hardware revisions.
There is no recommended 28 pin Ice version at this time.  (Revision A does exist, but is not recommended and support may have been removed from the latest firware revisions.)

## Notes on Fabrication

All recommended designs have been manufactured and assembled using JLCPCB's PCB assembly service.  The gerbers and BOM files in each revision's `fab/` directory are compatible with JLCPCB's requirements.  However, you need to exercise care when submitting the order to ensure the correct options are selected.  In particular:

- Select economic assembly, not standard assembly, for cost reasons.
- Use standard 1.6mm PCB thickness.  You probably want HASL with lead finish for cost reasons.
- You have to select the desired PCB colour as part of the PCB ordering process.  Red is recommended for Fire and blue for Ice.
- As of late 2025 you no longer need to remove JLC's order ID from the PCBs, but it is worth checking this is still the case when you place your order.
- After uploading the BOM and position files for assembly, you need to ensure all parts are available and checked.
- You must ensure JLC is showing the correct orientation for all components.  There are silkscreen markings for pin 1 on all ICs and polarized components - ensure this matches the pink dot of the component in the viewer.  Also ensure the USB connector is the correct orientation.

Occasionally, some chosen parts are out of stock on JLCPCB.  One ROM has been designed with extremely common parts where possible, which JLC tend to hold large stocks of.  However, if a part is unavailable you may need to select an alternative part.  Ensure the alternative part has the same footprint and electrical characteristics (e.g. capacitance, voltage rating, etc) as the original part.  If in doubt, query within github discussions for advice. 

JLC may raise an issue with you after submitting your order.  Common issues include:
- JLC may ask to add side rails with stamp holes, to help with the PCB assembly process.  While One ROM has been successfully fabbed without these, if JLC recommends them it is best to accept this change, as otherwise they may not warrant the assembled boards.  The side rails can be snapped off using the "mouse bites" (stamp holes) after assembly.  JLC may or may not accept replacing stamp holes with v-score - as v-score is not available on their economic service.

If in doubt, query within github discussions for advice.

Assembly errors:
- Ice boards have rarely, if ever, experienced assembly errors.
- The first few batches of Fire boards experienced some assembly issues, particularly with respect to the RP2350 MCU solder pad fillets.  More recent boards and orders have been without issue.  It is unclear whether differences in the later PCBs or improvements in JLC's assembly process are responsible for this improvement.  If you do experience assembly issues, raise with JLC support.  It would also be interesting to hear on github discussions what issues you experienced, so others can be aware.

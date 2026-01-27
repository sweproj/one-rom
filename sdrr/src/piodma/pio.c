// Copyright (C) 2026 Piers Finlayson <piers@piers.rocks>
//
// MIT License

// RP2350 Shared PIO routines

#include "include.h"

#if defined(RP235X)

#include "piodma/piodma.h"

void pio(
    const sdrr_info_t *info,
    const sdrr_rom_set_t *set,
    uint32_t rom_table_addr
) {
    if (set->roms[0]->rom_type == CHIP_TYPE_6116) {
        DEBUG("PIO RAM Mode");
        pioram(info, rom_table_addr);
    } else {
        DEBUG("PIO ROM Mode");
        piorom(info, set, rom_table_addr);
    }
}

#endif // RP235X
// Contains enums used by the One ROM firmware.

// Copyright (C) 2025 Piers Finlayson <piers@piers.rocks>
//
// MIT License

#ifndef ENUMS_H
#define ENUMS_H

typedef enum {
    F401DE = 0x0000,  // 96 KB RAM
    F405 = 0x0001,
    F411 = 0x0002,
    F446 = 0x0003,
    F401BC = 0x0004,  // Only 64KB RAM
    RP2350_LINE = 0x0005,
    MCU_LINE_FORCE_UINT16 = 0xFFFF,
} mcu_line_t;
_Static_assert(sizeof(mcu_line_t) == 2, "mcu_line_t must be 2 bytes");

typedef enum {
    STORAGE_8 = 0x00,
    STORAGE_B = 0x01,
    STORAGE_C = 0x02,
    STORAGE_D = 0x03,
    STORAGE_E = 0x04,
    STORAGE_F = 0x05,
    STORAGE_G = 0x06,
    STORAGE_2MB = 0x07,
    MCU_STORAGE_FORFCE_UINT16 = 0xFFFF,
} mcu_storage_t;
_Static_assert(sizeof(mcu_storage_t) == 2, "mcu_storage_t must be 2 bytes");

// Only ports A-D are exposed on the 64-pin STM32F4s.
// RP2350 has port (bank) 0.
typedef enum {
    PORT_NONE = 0x00,
    PORT_A    = 0x01,
    PORT_B    = 0x02,
    PORT_C    = 0x03,
    PORT_D    = 0x04,
    PORT_0    = 0x05,  // RP2350
} sdrr_mcu_port_t;
_Static_assert(sizeof(sdrr_mcu_port_t) == 1, "sdrr_mcu_port_t must be 1 byte");

// Supported RP2350 clock frequencies
#define FIRE_FREQ_STOCK  0xffff
#define FIRE_FREQ_NONE   0
typedef uint16_t fire_freq_t;
_Static_assert(sizeof(fire_freq_t) == 2, "fire_freq_t must be 2 byte");

// Supported STM32F4 clock frequencies
#define ICE_FREQ_STOCK  0xffff
#define ICE_FREQ_NONE   0
typedef uint16_t ice_freq_t;
_Static_assert(sizeof(ice_freq_t) == 2, "ice_freq_t must be 2 byte");

typedef enum {
    FIRE_VREG_0_55V = 0x00,
    FIRE_VREG_0_60V = 0x01,
    FIRE_VREG_0_65V = 0x02,
    FIRE_VREG_0_70V = 0x03,
    FIRE_VREG_0_75V = 0x04,
    FIRE_VREG_0_80V = 0x05,
    FIRE_VREG_0_85V = 0x06,
    FIRE_VREG_0_90V = 0x07,
    FIRE_VREG_0_95V = 0x08,
    FIRE_VREG_1_00V = 0x09,
    FIRE_VREG_1_05V = 0x0A,
    FIRE_VREG_1_10V = 0x0B,
    FIRE_VREG_1_15V = 0x0C,
    FIRE_VREG_1_20V = 0x0D,
    FIRE_VREG_1_25V = 0x0E,
    FIRE_VREG_1_30V = 0x0F,
    FIRE_VREG_1_35V = 0x10,
    FIRE_VREG_1_40V = 0x11,
    FIRE_VREG_1_50V = 0x12,
    FIRE_VREG_1_60V = 0x13,
    FIRE_VREG_1_65V = 0x14,
    FIRE_VREG_1_70V = 0x15,
    FIRE_VREG_1_80V = 0x16,
    FIRE_VREG_1_90V = 0x17,
    FIRE_VREG_2_00V = 0x18,
    FIRE_VREG_2_35V = 0x19,
    FIRE_VREG_2_50V = 0x1A,
    FIRE_VREG_2_65V = 0x1B,
    FIRE_VREG_2_80V = 0x1C,
    FIRE_VREG_3_00V = 0x1D,
    FIRE_VREG_3_15V = 0x1E,
    FIRE_VREG_3_30V = 0x1F,
    FIRE_VREG_NONE = 0xFE,
    FIRE_VREG_STOCK = 0xFF,
} fire_vreg_t;
_Static_assert(sizeof(fire_vreg_t) == 1, "fire_vreg_t must be 1 byte");

#endif // ENUMS_H

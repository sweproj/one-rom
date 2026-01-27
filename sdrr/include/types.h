// Contains types

// Copyright (C) 2025 Piers Finlayson <piers@piers.rocks>
//
// MIT License

#ifndef TYPES_H
#define TYPES_H

// Blink patterns for limp mode
typedef enum limp_mode_pattern: uint8_t {
    LIMP_MODE_NONE = 0,
    LIMP_MODE_NO_ROMS = 1,
    LIMP_MODE_INVALID_CONFIG = 2,
    LIMP_MODE_INVALID_BUILD = 3,
    NUM_LIMP_MODE_PATTERNS 
} limp_mode_pattern_t;

typedef struct {
    uint32_t on_time;
    uint32_t off_time;
} limp_mode_info_t;

extern const limp_mode_info_t limp_mode_patterns[NUM_LIMP_MODE_PATTERNS];
_Static_assert(sizeof(limp_mode_patterns)/sizeof(limp_mode_patterns[0]) == NUM_LIMP_MODE_PATTERNS, 
               "limp_mode_patterns array size mismatch");
#if defined(ONEROM_CONSTANTS)
// Define the limp mode patterns
//
// Exact speed depends on clock speed and that depends on where in the boot
// cycle we are.  Use the relative lengths to identify the pattern.
const limp_mode_info_t limp_mode_patterns[NUM_LIMP_MODE_PATTERNS] = {
    {100000, 500000},       // LIMP_MODE_NONE
    {5000000, 25000000},    // LIMP_MODE_NO_ROMS
    {1000000, 1000000},     // LIMP_MODE_INVALID_CONFIG
    {25000000, 500000},     // LIMP_MODE_INVALID_BUILD
};
#endif // ONEROM_CONSTANTS

#endif // TYPES_H

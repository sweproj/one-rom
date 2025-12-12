// Query generated roms.c

// Copyright (C) 2025 Piers Finlayson <piers@piers.rocks>
//
// MIT License

#include "roms-test.h"
#include "json-config.h"

static struct {
    uint8_t addr_pins[MAX_ADDR_LINES];
    uint8_t cs1_pin;
    uint8_t x1_pin;
    uint8_t x2_pin;
    int initialized;
} address_mangler = {0};

void create_address_mangler(json_config_t* config) {
    // Assert CS1 is same for all ROM types
    if (config->rom.pin_count == 24 ) {
        assert(config->mcu.pins.cs1.pin_2364 == config->mcu.pins.cs1.pin_2332);
        assert(config->mcu.pins.cs1.pin_2332 == config->mcu.pins.cs1.pin_2316);
        assert(config->mcu.pins.cs1.pin_2364 != 255);
    }
    
    memcpy(address_mangler.addr_pins, config->mcu.pins.addr, sizeof(address_mangler.addr_pins));
    if (config->rom.pin_count == 24 ) {
        address_mangler.cs1_pin = config->mcu.pins.cs1.pin_2364;
    } else {
        address_mangler.cs1_pin = config->mcu.pins.cs1.pin_23128;
        if (address_mangler.cs1_pin == 255) {
            address_mangler.cs1_pin = config->mcu.pins.ce.pin_27128;
        }
    }
    address_mangler.x1_pin = config->mcu.pins.x1;
    address_mangler.x2_pin = config->mcu.pins.x2;
    address_mangler.initialized = 1;

    if (config->rom.pin_count == 24) {
        if ((config->mcu.ports.data_port == config->mcu.ports.addr_port) && (config->mcu.pins.data[0] < 8)) {
            // If data and address ports are the same, and data lines are 0-7, then
            // address lines must be 8-23.  Subtract 8 off them so thare are 0-15.
            for (int ii = 0; ii < MAX_ADDR_LINES; ii++) {
                if (address_mangler.addr_pins[ii] != 255) {
                    address_mangler.addr_pins[ii] -= 8;
                }
            }

            // And the CS and X lines too
            if (address_mangler.cs1_pin != 255) {
                address_mangler.cs1_pin -= 8;
            }
            if (address_mangler.x1_pin != 255) {
                address_mangler.x1_pin -= 8;
            }
            if (address_mangler.x2_pin != 255) {
                address_mangler.x2_pin -= 8;
            }
        }
    } else {
        // CS pins are not part of address space for 28 pin ROMs, but we do
        // need to left shift address pins
        
        // Find the minimum address pin
        uint8_t min_addr_pin = 255;
        for (int ii = 0; ii < MAX_ADDR_LINES; ii++) {
            if (address_mangler.addr_pins[ii] < min_addr_pin) {
                min_addr_pin = address_mangler.addr_pins[ii];
            }
        }

        // Now subtract it off all address pins
        for (int ii = 0; ii < MAX_ADDR_LINES; ii++) {
            if (address_mangler.addr_pins[ii] != 255) {
                address_mangler.addr_pins[ii] -= min_addr_pin;
            }
        }
    }
}

static struct {
    uint8_t data_pins[NUM_DATA_LINES];
    int initialized;
} byte_demangler = {0};

void create_byte_demangler(json_config_t* config) {
    memcpy(byte_demangler.data_pins, config->mcu.pins.data, sizeof(byte_demangler.data_pins));
    if (!strcmp(config->mcu.family, "rp2350")) {
        for (int ii = 0; ii < NUM_DATA_LINES; ii++) {
            // RP2350 uses a higher byte for data lines, but still expects to
            // read a single byte at a time - the RP2350 hardware takes care
            // of getting the value shifted. 
            byte_demangler.data_pins[ii] = byte_demangler.data_pins[ii] % 8;
        }
    }
    byte_demangler.initialized = 1;
}

// lookup_rom_byte - Simulates the lookup of a byte from the ROM image based on the mangled address
uint8_t lookup_rom_byte(uint8_t set, uint16_t mangled_addr) {  // Removed unused CS parameters
    return rom_set[set].data[mangled_addr];
}

uint16_t create_mangled_address(size_t rom_pins, uint16_t logical_addr, int cs1, int x1, int x2) {
    assert(address_mangler.initialized);
    
    uint16_t mangled = 0;
    
    if (rom_pins == 24) {
        assert(address_mangler.cs1_pin <= 15);
        assert(address_mangler.x1_pin <= 15);
        assert(address_mangler.x2_pin <= 15);

        // Set CS selection bits (active low)
        if (cs1) mangled |= (1 << address_mangler.cs1_pin);
        if (x1)  mangled |= (1 << address_mangler.x1_pin);  
        if (x2)  mangled |= (1 << address_mangler.x2_pin);
    }
    
    // Map logical address bits to configured GPIO positions
    for (int i = 0; i < MAX_ADDR_LINES; i++) {
        if (logical_addr & (1 << i)) {
            assert(address_mangler.addr_pins[i] <= 15);
            mangled |= (1 << address_mangler.addr_pins[i]);
        }
    }

    return mangled;
}

uint8_t demangle_byte(uint8_t mangled_byte) {
    assert(byte_demangler.initialized);
    
    uint8_t logical = 0;
    
    for (int i = 0; i < NUM_DATA_LINES; i++) {
        assert(byte_demangler.data_pins[i] <= 7);
        if (mangled_byte & (1 << byte_demangler.data_pins[i])) {
            logical |= (1 << i);
        }
    }

    return logical;
}

// Convert ROM type number to string
const char* rom_type_to_string(int rom_type) {
    switch (rom_type) {
        case ROM_TYPE_2316: return "2316";
        case ROM_TYPE_2332: return "2332";  
        case ROM_TYPE_2364: return "2364";
        case ROM_TYPE_23128: return "23128";
        case ROM_TYPE_23256: return "23256";
        case ROM_TYPE_23512: return "23512";
        case ROM_TYPE_2716: return "2716";
        case ROM_TYPE_2732: return "2732";
        case ROM_TYPE_2764: return "2764";
        case ROM_TYPE_27128: return "27128";
        case ROM_TYPE_27256: return "27256";
        case ROM_TYPE_27512: return "27512";
        default: return "unknown";
    }
}

// Convert CS state number to string
const char* cs_state_to_string(int cs_state) {
    switch (cs_state) {
        case CS_ACTIVE_LOW: return "active_low";
        case CS_ACTIVE_HIGH: return "active_high";
        case CS_NOT_USED: return "not_used";
        default: return "unknown";
    }
}

// Get expected ROM size for type
size_t get_expected_rom_size(int rom_type) {
    switch (rom_type) {
        case ROM_TYPE_2316: return 2048;
        case ROM_TYPE_2332: return 4096;
        case ROM_TYPE_2364: return 8192;
        case ROM_TYPE_23128: return 16384;
        case ROM_TYPE_23256: return 32768;
        case ROM_TYPE_23512: return 65536;
        case ROM_TYPE_2716: return 2048;
        case ROM_TYPE_2732: return 4096;
        case ROM_TYPE_2764: return 8192;
        case ROM_TYPE_27128: return 16384;
        case ROM_TYPE_27256: return 32768;
        case ROM_TYPE_27512: return 65536;
        default: return 0;
    }
}

int rom_type_from_string(const char* type_str) {
    if (strcmp(type_str, "2316") == 0) return ROM_TYPE_2316;
    else if (strcmp(type_str, "2332") == 0) return ROM_TYPE_2332;
    else if (strcmp(type_str, "2364") == 0) return ROM_TYPE_2364;
    else if (strcmp(type_str, "23128") == 0) return ROM_TYPE_23128;
    else if (strcmp(type_str, "23256") == 0) return ROM_TYPE_23256;
    else if (strcmp(type_str, "23512") == 0) return ROM_TYPE_23512;
    else if (strcmp(type_str, "2704") == 0) return ROM_TYPE_2704;
    else if (strcmp(type_str, "2708") == 0) return ROM_TYPE_2708;
    else if (strcmp(type_str, "2716") == 0) return ROM_TYPE_2716;
    else if (strcmp(type_str, "2732") == 0) return ROM_TYPE_2732;
    else if (strcmp(type_str, "2764") == 0) return ROM_TYPE_2764;
    else if (strcmp(type_str, "27128") == 0) return ROM_TYPE_27128;
    else if (strcmp(type_str, "27256") == 0) return ROM_TYPE_27256;
    else if (strcmp(type_str, "27512") == 0) return ROM_TYPE_27512;
    else return -1; // Unknown type
}

void print_compiled_rom_info(void) {
    printf("\n=== Compiled ROM Sets Analysis ===\n");
    printf("Total ROM images: %d\n", SDRR_NUM_IMAGES);
    printf("Total ROM sets: %d\n", sdrr_rom_set_count);
    
    // Print details for each ROM set
    for (int set_idx = 0; set_idx < sdrr_rom_set_count; set_idx++) {
        printf("\nROM Set %d:\n", set_idx);
        printf("  Size: %u bytes (%s)\n", rom_set[set_idx].size, 
               (rom_set[set_idx].size == 16384) ? "16KB" : 
               (rom_set[set_idx].size == 65536) ? "64KB" : "other");
        printf("  ROM count: %d\n", rom_set[set_idx].rom_count);
        
        // Expected image size based on ROM count
        const char* expected_size = (rom_set[set_idx].rom_count == 1) ? "16KB" : "64KB";
        printf("  Expected size: %s", expected_size);
        if ((rom_set[set_idx].rom_count == 1 && rom_set[set_idx].size == 16384) ||
            (rom_set[set_idx].rom_count > 1 && rom_set[set_idx].size == 65536)) {
            printf(" ✓\n");
        } else {
            printf(" ✗\n");
        }
        
        // Print details for each ROM in this set
        for (int rom_idx = 0; rom_idx < rom_set[set_idx].rom_count; rom_idx++) {
            const sdrr_rom_info_t *rom_info = rom_set[set_idx].roms[rom_idx];
            
            printf("  ROM %d:\n", rom_idx);
#if defined(BOOT_LOGGING)
            printf("    File: %s\n", rom_info->filename);
#endif // BOOT_LOGGING
            printf("    Type: %s (%d)\n", rom_type_to_string(rom_info->rom_type), rom_info->rom_type);
            printf("    CS1: %s (%d)", cs_state_to_string(rom_info->cs1_state), rom_info->cs1_state);
            
            if (rom_info->cs2_state != CS_NOT_USED) {
                printf(", CS2: %s (%d)", cs_state_to_string(rom_info->cs2_state), rom_info->cs2_state);
            }
            if (rom_info->cs3_state != CS_NOT_USED) {
                printf(", CS3: %s (%d)", cs_state_to_string(rom_info->cs3_state), rom_info->cs3_state);
            }
            printf("\n");
            
            // Expected ROM size check
            size_t expected_rom_size = get_expected_rom_size(rom_info->rom_type);
            printf("    Expected ROM size: %zu bytes\n", expected_rom_size);
        }
        
        // Show first 8 bytes of the ROM set data
        printf("  First 8 bytes of mangled set data: ");
        for (size_t j = 0; j < 8 && j < rom_set[set_idx].size; j++) {
            printf("0x%02X ", rom_set[set_idx].data[j]);
        }
        printf("\n");
    }
}

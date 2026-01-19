// Copyright (C) 2026 Piers Finlayson <piers@piers.rocks>
//
// MIT License

// RP2350 PIO Disassembler
//
// Used by DEBUG_LOGGING=1 to decode PIO SM programs for logging purposes.

#include "include.h"

#if defined(RP235X)

#include "piodma/piodma.h"

#if defined(DEBUG_LOGGING) && (DEBUG_LOGGING == 1)

static const char* piorom_get_jmp_condition(uint8_t cond) {
    switch (cond) {
        case 0b000: return "";
        case 0b001: return "!x";
        case 0b010: return "x--";
        case 0b011: return "!y";
        case 0b100: return "y--";
        case 0b101: return "x!=y";
        case 0b110: return "pin";
        case 0b111: return "!osre";
        default: return "???";
    }
}

static const char* piorom_get_wait_source(uint8_t src) {
    switch (src) {
        case 0b00: return "gpio";
        case 0b01: return "pin";
        case 0b10: return "irq";
        case 0b11: return "jmppin";
        default: return "???";
    }
}

static const char* piorom_get_in_source(uint8_t src) {
    switch (src) {
        case 0b000: return "pins";
        case 0b001: return "x";
        case 0b010: return "y";
        case 0b011: return "null";
        case 0b100: return "reserved";
        case 0b101: return "reserved";
        case 0b110: return "isr";
        case 0b111: return "osr";
        default: return "???";
    }
}

static const char* piorom_get_out_dest(uint8_t dest) {
    switch (dest) {
        case 0b000: return "pins";
        case 0b001: return "x";
        case 0b010: return "y";
        case 0b011: return "null";
        case 0b100: return "pindirs";
        case 0b101: return "pc";
        case 0b110: return "isr";
        case 0b111: return "exec";
        default: return "???";
    }
}

static const char* piorom_get_mov_dest(uint8_t dest) {
    switch (dest) {
        case 0b000: return "pins";
        case 0b001: return "x";
        case 0b010: return "y";
        case 0b011: return "pindirs";
        case 0b100: return "exec";
        case 0b101: return "pc";
        case 0b110: return "isr";
        case 0b111: return "osr";
        default: return "???";
    }
}

static const char* piorom_get_mov_op(uint8_t op) {
    switch (op) {
        case 0b00: return "";
        case 0b01: return "~";
        case 0b10: return "::";
        case 0b11: return "reserved";
        default: return "???";
    }
}

static const char* piorom_get_mov_source(uint8_t src) {
    switch (src) {
        case 0b000: return "pins";
        case 0b001: return "x";
        case 0b010: return "y";
        case 0b011: return "null";
        case 0b100: return "reserved";
        case 0b101: return "status";
        case 0b110: return "isr";
        case 0b111: return "osr";
        default: return "???";
    }
}

static const char* piorom_get_set_dest(uint8_t dest) {
    switch (dest) {
        case 0b000: return "pins";
        case 0b001: return "x";
        case 0b010: return "y";
        case 0b011: return "reserved";
        case 0b100: return "pindirs";
        case 0b101: return "reserved";
        case 0b110: return "reserved";
        case 0b111: return "reserved";
        default: return "???";
    }
}

static char* append_str(char* dest, const char* src) {
    while (*src) {
        *dest++ = *src++;
    }
    return dest;
}

static char* append_char(char* dest, char c) {
    *dest++ = c;
    return dest;
}

static char* append_uint(char* dest, uint32_t val) {
    if (val == 0) {
        *dest++ = '0';
        return dest;
    }
    
    char temp[11];
    int i = 0;
    while (val > 0) {
        temp[i++] = '0' + (val % 10);
        val /= 10;
    }
    
    while (i > 0) {
        *dest++ = temp[--i];
    }
    return dest;
}

static char* append_delay(char* dest, uint8_t delay) {
    if (delay > 0) {
        dest = append_str(dest, " [");
        dest = append_uint(dest, delay);
        dest = append_char(dest, ']');
    }
    return dest;
}

void pio_instruction_decoder(uint32_t instr, char out_str[64], uint8_t start_offset) {
    uint8_t opcode = (instr >> 13) & 0x7;
    uint8_t delay = (instr >> 8) & 0x1F;
    char* p;
    
    switch (opcode) {
        case 0b000: { // JMP
            uint8_t condition = (instr >> 5) & 0x7;
            uint8_t address = instr & 0x1F;
            p = out_str;
            p = append_str(p, "jmp ");
            if (condition != 0) {
                p = append_str(p, piorom_get_jmp_condition(condition));
                p = append_str(p, ", ");
            }
            p = append_uint(p, address - start_offset);
            p = append_delay(p, delay);
            *p = '\0';
            break;
        }
        
        case 0b001: { // WAIT
            uint8_t pol = (instr >> 7) & 0x1;
            uint8_t source = (instr >> 5) & 0x3;
            uint8_t idx_mode = (instr >> 3) & 0x3;
            p = out_str;
            p = append_str(p, "wait ");
            p = append_uint(p, pol);
            p = append_char(p, ' ');
            p = append_str(p, piorom_get_wait_source(source));

            uint8_t index;
            if (source == 0b10) {
                // IRQ
                index = instr & 0x7;

                // prev/next
                if (idx_mode == 0b01) {
                    p = append_str(p, " prev");
                } else if (idx_mode == 0b11) {
                    p = append_str(p, " next");
                }
            } else {
                // Other
                index = instr & 0x1F;
            }


            p = append_str(p, " ");
            p = append_uint(p, index);
            p = append_delay(p, delay);
            *p = '\0';
            break;
        }
        
        case 0b010: { // IN
            uint8_t source = (instr >> 5) & 0x7;
            uint8_t bitcount = instr & 0x1F;
            p = out_str;
            p = append_str(p, "in ");
            p = append_str(p, piorom_get_in_source(source));
            p = append_str(p, ", ");
            p = append_uint(p, bitcount);
            p = append_delay(p, delay);
            *p = '\0';
            break;
        }
        
        case 0b011: { // OUT
            uint8_t dest = (instr >> 5) & 0x7;
            uint8_t bitcount = instr & 0x1F;
            p = out_str;
            p = append_str(p, "out ");
            p = append_str(p, piorom_get_out_dest(dest));
            p = append_str(p, ", ");
            p = append_uint(p, bitcount);
            p = append_delay(p, delay);
            *p = '\0';
            break;
        }
        
        case 0b100: { // PUSH/PULL/MOV indexed
            uint8_t bit7 = (instr >> 7) & 0x1;
            uint8_t bit4 = (instr >> 4) & 0x1;
            p = out_str;
            
            if (bit4 == 0) {
                // PUSH or PULL
                uint8_t if_flag = (instr >> 6) & 0x1;
                uint8_t block = (instr >> 5) & 0x1;
                
                if (bit7 == 0) {
                    // PUSH
                    p = append_str(p, "push");
                    if (if_flag) {
                        p = append_str(p, " iffull ");
                    } else {
                        p = append_char(p, ' ');
                    }
                    p = append_str(p, block ? "block" : "noblock");
                } else {
                    // PULL
                    p = append_str(p, "pull");
                    if (if_flag) {
                        p = append_str(p, " ifempty ");
                    } else {
                        p = append_char(p, ' ');
                    }
                    p = append_str(p, block ? "block" : "noblock");
                }
            } else {
                // MOV indexed
                uint8_t idx_i = (instr >> 3) & 0x1;
                uint8_t index = instr & 0x3;
                
                if (bit7 == 0) {
                    // MOV RX
                    p = append_str(p, "mov rxfifo[");
                    if (idx_i) {
                        p = append_uint(p, index);
                    } else {
                        p = append_char(p, 'y');
                    }
                    p = append_str(p, "], isr");
                } else {
                    // MOV TX
                    p = append_str(p, "mov txfifo[");
                    if (idx_i) {
                        p = append_uint(p, index);
                    } else {
                        p = append_char(p, 'y');
                    }
                    p = append_str(p, "], osr");
                }
            }
            p = append_delay(p, delay);
            *p = '\0';
            break;
        }
        
        case 0b101: { // MOV
            uint8_t dest = (instr >> 5) & 0x7;
            uint8_t op = (instr >> 3) & 0x3;
            uint8_t source = instr & 0x7;
            p = out_str;
            
            // Check for nop (mov y, y)
            if (dest == 0b010 && op == 0b00 && source == 0b010) {
                p = append_str(p, "nop");
            } else {
                p = append_str(p, "mov ");
                p = append_str(p, piorom_get_mov_dest(dest));
                p = append_str(p, ", ");
                p = append_str(p, piorom_get_mov_op(op));
                p = append_str(p, piorom_get_mov_source(source));
            }
            p = append_delay(p, delay);
            *p = '\0';
            break;
        }
        
        case 0b110: { // IRQ
            uint8_t clr = (instr >> 6) & 0x1;
            uint8_t wait = (instr >> 5) & 0x1;
            uint8_t idx_mode = (instr >> 3) & 0x3;
            uint8_t index = instr & 0x7;
            p = out_str;
            p = append_str(p, "irq ");
            
            // prev/next
            if (idx_mode == 0b01) {
                p = append_str(p, "prev ");
            } else if (idx_mode == 0b11) {
                p = append_str(p, "next ");
            }
            
            // set/wait/clear
            if (clr) {
                p = append_str(p, "clear ");
            } else if (wait) {
                p = append_str(p, "wait ");
            }
            
            p = append_uint(p, index);
            
            // rel
            if (idx_mode == 0b10) {
                p = append_str(p, " rel");
            }
            
            p = append_delay(p, delay);
            *p = '\0';
            break;
        }
        
        case 0b111: { // SET
            uint8_t dest = (instr >> 5) & 0x7;
            uint8_t data = instr & 0x1F;
            p = out_str;
            p = append_str(p, "set ");
            p = append_str(p, piorom_get_set_dest(dest));
            p = append_str(p, ", ");
            p = append_uint(p, data);
            p = append_delay(p, delay);
            *p = '\0';
            break;
        }
    }
}

// Log the PIO state machine configuration and program instructions
// Arguments:
// - sm_name: Human readable name of the state machine/program
// - pio_block: PIO block number (0, 1, or 2)
// - pio_sm: PIO state machine number (0-3)
// - instr_scratch: Pointer to the full array of instructions for this block
// - first_instr: Index of the first instruction of this program in instr_scratch
// - start: Index of the .start instruction (where the SM starts execution)
// - end: Index of the last instruction of this program
void pio_log_sm(
    const char *sm_name,
    uint8_t pio_block,
    uint8_t pio_sm,
    uint16_t *instr_scratch,
    uint8_t first_instr,
    uint8_t start,
    uint8_t end
) {
    volatile pio_sm_reg_t *sm_reg;
    char instr[64];
    
    // Get the correct PIO block's SM register
    if (pio_block == 0) {
        sm_reg = PIO0_SM_REG(pio_sm);
    } else if (pio_block == 1) {
        sm_reg = PIO1_SM_REG(pio_sm);
    } else {
        sm_reg = PIO2_SM_REG(pio_sm);
    }
    
    DEBUG("PIO%d:%d %s (%d instructions)", pio_block, pio_sm, sm_name, (end - first_instr + 1));

    uint16_t clkdiv_int = PIO_CLKDIV_INT_FROM_REG(sm_reg->clkdiv);
    uint8_t clkdiv_frac = PIO_CLKDIV_FRAC_FROM_REG(sm_reg->clkdiv);
    uint8_t wrap_bottom = PIO_WRAP_BOTTOM_FROM_REG(sm_reg->execctrl);
    uint8_t wrap_top = PIO_WRAP_TOP_FROM_REG(sm_reg->execctrl);
    DEBUG(
        "  CLKDIV: %d.%02d EXECCTRL: 0x%08X SHIFTCTRL: 0x%08X PINCTRL: 0x%08X",
        clkdiv_int,
        clkdiv_frac,
        sm_reg->execctrl,
        sm_reg->shiftctrl,
        sm_reg->pinctrl
    );
    DEBUG("  .program pio%d_sm%d", pio_block, pio_sm);
    for (int ii = first_instr; ii <= end; ii++) {
        if (ii == start) {
            DEBUG("  .start");
        }
        if (ii == wrap_bottom) {
            DEBUG("  .wrap_target");
        }
        pio_instruction_decoder(instr_scratch[ii], instr, first_instr);
        DEBUG("    %d: 0x%04X ; %s", ii - first_instr, instr_scratch[ii], instr);
        if (ii == wrap_top) {
            DEBUG("  .wrap");
        }
    }
}

#endif // defined(DEBUG_LOGGING)
#endif // defined(RP235X)
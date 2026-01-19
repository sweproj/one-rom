// Copyright (C) 2026 Piers Finlayson <piers@piers.rocks>
//
// MIT License

// RP2350 PIO/DMA autonomous RAM serving support

#include "include.h"

#if defined(RP235X)

#include "piodma/piodma.h"

//
// Config options
//

// Number of checks to confirm /W is active.  Can we used to debounce noisy /W
// signals, or brief /W low glitches.
#define PIORAM_WRITE_ACTIVE_CHECK_MAX 8  // Too high and we'll run out of instructions
#define PIORAM_WRITE_ACTIVE_CHECK_MIN 1
#ifndef PIORAM_WRITE_ACTIVE_CHECK_COUNT
#define PIORAM_WRITE_ACTIVE_CHECK_COUNT 2
#endif // PIORAM_WRITE_ACTIVE_CHECK_COUNT

#ifndef PIORAM_WRITE_TRIGGER_IRQ_DELAY
// Number of cycles to delay after triggering RAM WRITE IRQ before checking
// whether /W has gone high.  This provides time for the data and address
// reader SMs to get into a state where they can check /W as well.
#define PIORAM_WRITE_TRIGGER_IRQ_DELAY 4
#endif // PIORAM_WRITE_TRIGGER_IRQ_DELAY

// The IRQ number used to trigger RAM WRITE handling.  The PIO block used for
// this IRQ is the PIO block where the Data read handler SM is located (i.e.
// the SM that triggers the IRQ when /W goes low).
#define RAM_WRITE_TRIGGER_IRQ  3

// Configuration structure for PIO RAM serving
typedef struct pioram_config {
    // CS pin configuration for READ (/CE and /OE)
    uint8_t read_cs_base_pin;
    uint8_t num_read_cs_pins;  // Should be 2 for 6116

    // CS pin configuration for WRITE (/CE and /W)
    uint8_t write_cs_base_pin;
    uint8_t num_write_cs_pins;  // Should be 2 for 6116

    // /W pin number
    uint8_t write_pin;
    uint8_t pad0[3];
    
    // Data pins (Q0-Q7)
    uint8_t data_base_pin;
    uint8_t num_data_pins;  // 8 for 6116
    
    // Address pins (A0-A10)
    uint8_t addr_base_pin;
    uint8_t num_addr_pins;  // 11 for 6116 (2KB)
    
    // RAM table base address in SRAM
    uint32_t ram_table_addr;

    // Clock dividers for each SM
    uint16_t data_read_handler_clkdiv_int;
    uint8_t data_read_handler_clkdiv_frac;
    uint8_t pad1;
    
    uint16_t addr_reader_read_clkdiv_int;
    uint8_t addr_reader_read_clkdiv_frac;
    uint8_t pad2;
    
    uint16_t addr_reader_write_clkdiv_int;
    uint8_t addr_reader_write_clkdiv_frac;
    uint8_t pad3;

    uint16_t data_io_clkdiv_int;
    uint8_t data_io_clkdiv_frac;
    uint8_t pad4;
    
    uint16_t data_out_clkdiv_int;
    uint8_t data_out_clkdiv_frac;
    uint8_t pad5;
    
    uint16_t data_in_clkdiv_int;
    uint8_t data_in_clkdiv_frac;
    uint8_t pad6;
} pioram_config_t;

// Function prototypes
static void pioram_load_programs(pioram_config_t *config);
static void pioram_setup_dma(pioram_config_t *config);
static void pioram_set_gpio_func(pioram_config_t *config);
static void pioram_start_pios(void);

// Build and load the PIO programs for RAM serving
//
// Uses the single-pass PIO assembler macros from pioasm.h
static void pioram_load_programs(pioram_config_t *config) {
    // Get the high X bits of the RAM table address for preloading into the
    // address reader SMs.
    uint8_t ram_table_num_addr_bits = 32 - config->num_addr_pins;
    uint32_t high_bits_mask = (1 << ram_table_num_addr_bits) - 1;
    uint32_t low_bits_mask = (1 << config->num_addr_pins) - 1;
    uint32_t __attribute__((unused)) alignment_size = (1 << config->num_addr_pins) / 1024;
    DEBUG("Checking RAM table address 0x%08X is %uKB aligned", config->ram_table_addr, alignment_size);
    DEBUG("High bits mask: 0x%08X, low bits mask: 0x%08X", high_bits_mask, low_bits_mask);
    if (config->ram_table_addr & low_bits_mask) {
        LOG("!!! PIO RAM serving requires RAM table address to be %uKB aligned",
            alignment_size);
        limp_mode(LIMP_MODE_INVALID_CONFIG);
    }
    uint32_t ram_table_high_bits = (config->ram_table_addr >> config->num_addr_pins) & high_bits_mask;
    DEBUG("RAM table high %d bits: 0x%08X", ram_table_num_addr_bits, ram_table_high_bits);

#if defined(DEBUG_LOGGING)
    // Log other config values
    uint8_t read_cs_base_pin = config->read_cs_base_pin;
    uint8_t write_cs_base_pin = config->write_cs_base_pin;
    uint8_t num_read_cs_pins = config->num_read_cs_pins;
    uint8_t num_write_cs_pins = config->num_write_cs_pins;
    uint8_t write_pin = config->write_pin;
    uint8_t data_base_pin = config->data_base_pin;
    uint8_t num_data_pins = config->num_data_pins;
    uint8_t addr_base_pin = config->addr_base_pin;
    uint8_t num_addr_pins = config->num_addr_pins;
    uint16_t data_read_handler_clkdiv_int = config->data_read_handler_clkdiv_int;
    uint8_t data_read_handler_clkdiv_frac = config->data_read_handler_clkdiv_frac;
    uint16_t addr_reader_read_clkdiv_int = config->addr_reader_read_clkdiv_int;
    uint8_t addr_reader_read_clkdiv_frac = config->addr_reader_read_clkdiv_frac;
    uint16_t addr_reader_write_clkdiv_int = config->addr_reader_write_clkdiv_int;
    uint8_t addr_reader_write_clkdiv_frac = config->addr_reader_write_clkdiv_frac;
    uint16_t data_io_clkdiv_int = config->data_io_clkdiv_int;
    uint8_t data_io_clkdiv_frac = config->data_io_clkdiv_frac;
    uint16_t data_out_clkdiv_int = config->data_out_clkdiv_int;
    uint8_t data_out_clkdiv_frac = config->data_out_clkdiv_frac;
    uint16_t data_in_clkdiv_int = config->data_in_clkdiv_int;
    uint8_t data_in_clkdiv_frac = config->data_in_clkdiv_frac;
    DEBUG("PIO RAM Serving Config:");
    DEBUG("- /OE /CE pins: %d-%d", read_cs_base_pin, read_cs_base_pin + num_read_cs_pins - 1);
    DEBUG("- /CE /W pins: %d-%d", write_cs_base_pin, write_cs_base_pin + num_write_cs_pins - 1);
    DEBUG("- /W pin: %d", write_pin);
    DEBUG("- Data pins: %d-%d", data_base_pin, data_base_pin + num_data_pins - 1);
    DEBUG("- Address pins: %d-%d", addr_base_pin, addr_base_pin + num_addr_pins - 1);
    DEBUG("- Data Read Handler CLKDIV: %d.%02d", data_read_handler_clkdiv_int, data_read_handler_clkdiv_frac);
    DEBUG("- Addr Reader READ CLKDIV: %d.%02d", addr_reader_read_clkdiv_int, addr_reader_read_clkdiv_frac);
    DEBUG("- Addr Reader WRITE CLKDIV: %d.%02d", addr_reader_write_clkdiv_int, addr_reader_write_clkdiv_frac);
    DEBUG("- Data IO CLKDIV: %d.%02d", data_io_clkdiv_int, data_io_clkdiv_frac);
    DEBUG("- Data OUT CLKDIV: %d.%02d", data_out_clkdiv_int, data_out_clkdiv_frac);
    DEBUG("- Data IN CLKDIV: %d.%02d", data_in_clkdiv_int, data_in_clkdiv_frac);
#endif // DEBUG_LOGGGING

    // Set up the PIO assembler
    PIO_ASM_INIT();
    
    // Clear all PIO IRQs
    PIO_CLEAR_ALL_IRQS();

    // PIO0 Programs
    //
    // Combined data/address handlers
    PIO_SET_BLOCK(0);

    // SM0 - Data read handler - triggers data read chain on /CE and /W low
    //
    // Reads both /CE and /W together.  When both are low, triggers first the
    // WRITE address reader, then the data input reader.
    //
    // Re-arms once either /CE or /W goes high.
    PIO_SET_SM(0);

    PIO_LABEL_NEW(start_write_enabled_check);
    // This algorithm will check /CE and /W this number of times when it goes
    // low, to make sure it's really low.
    uint8_t data_read_check_count = PIORAM_WRITE_ACTIVE_CHECK_COUNT;
    if (data_read_check_count > PIORAM_WRITE_ACTIVE_CHECK_MAX) {
        data_read_check_count = PIORAM_WRITE_ACTIVE_CHECK_MAX;
        LOG("!!! PIORAM WE ACTIVE CHECK COUNT too high, limiting to %d", PIORAM_WRITE_ACTIVE_CHECK_MAX);
    } else if (data_read_check_count < PIORAM_WRITE_ACTIVE_CHECK_MIN) {
        data_read_check_count = 1;
        LOG("!!! PIORAM WE ACTIVE CHECK COUNT too low, setting to 1");
    }
    for (int ii = 0; ii < data_read_check_count; ii++) {
        // Read /CE and /W
        PIO_ADD_INSTR(MOV_X_PINS);
        
        // If either /CE or /W is high, check again
        PIO_ADD_INSTR(JMP_X_DEC(PIO_LABEL(start_write_enabled_check)));
    }

    // Trigger RAM WRITE IRQ. Triggers both addr and data readers
    PIO_ADD_INSTR(ADD_DELAY(IRQ_SET(RAM_WRITE_TRIGGER_IRQ), PIORAM_WRITE_TRIGGER_IRQ_DELAY)); 

    // Wait for either /CE or /W to go high
    PIO_LABEL_NEW(check_write_disabled);
    PIO_ADD_INSTR(MOV_X_PINS);

    // If both /CE or /W still low, keep waiting, otherwise jump to start
    PIO_WRAP_TOP();
    PIO_ADD_INSTR(JMP_NOT_X(PIO_LABEL(check_write_disabled)));

    // Set the various SM register values
    PIO_SM_CLKDIV_SET(
        config->data_read_handler_clkdiv_int,
        config->data_read_handler_clkdiv_frac
    );
    PIO_SM_EXECCTRL_SET(0);
    PIO_SM_SHIFTCTRL_SET(
        PIO_IN_COUNT(config->num_write_cs_pins) |   // Reading /CE and /W
        PIO_IN_SHIFTDIR_L
    );
    PIO_SM_PINCTRL_SET(
        PIO_IN_BASE(config->write_cs_base_pin)      // /CE and /W pins
    );

    // Jump to start and log
    PIO_SM_JMP_TO_START();
    PIO_LOG_SM("Trigger Data and Address Reader (RAM WRITE)");

    //
    // PIO 0 - End of block
    //
    PIO_END_BLOCK();

    // PIO1 Programs
    //
    // Address Readers
    PIO_SET_BLOCK(1);

    // PIO1 - Address Readers
    // 
    // SM0 - Address Reader (RAM READ)
    //
    // Constantly serves bytes to the READ DMA chain
    PIO_SET_SM(0);

    // Preload high bits of RAM table address to X - done via TX FIFO before
    // starting as SET(X) only supports 5 bits.

    // Pull high bits from X
    PIO_ADD_INSTR(IN_X(ram_table_num_addr_bits));

    // Read address lines and push to RX FIFO, so READ DMA chain serves the
    // byte.  We add a delay after this, to avoid overloading the DMA chain.
    PIO_WRAP_TOP();
    PIO_ADD_INSTR(ADD_DELAY(IN_PINS(config->num_addr_pins), 2));   // Autopush

    // SM configuration
    PIO_SM_CLKDIV_SET(
        config->addr_reader_read_clkdiv_int,
        config->addr_reader_read_clkdiv_frac
    );
    PIO_SM_EXECCTRL_SET(0);
    PIO_SM_SHIFTCTRL_SET(
        PIO_IN_COUNT(config->num_addr_pins) |
        PIO_AUTOPUSH |          // Auto push when we hit threshold
        PIO_PUSH_THRESH(32) |   // Push when we have total of 32 bits (a full address)
        PIO_IN_SHIFTDIR_L |
        PIO_OUT_SHIFTDIR_L
    );
    PIO_SM_PINCTRL_SET(
        PIO_IN_BASE(config->addr_base_pin)
    );

    // Preload the X register to the high bits of the RAM table address
    PIO_TXF = ram_table_high_bits;
    PIO_SM_EXEC_INSTR(PULL_BLOCK);
    PIO_SM_EXEC_INSTR(MOV_X_OSR);

    // Jump to start and log
    PIO_SM_JMP_TO_START();
    PIO_LOG_SM("Address Reader (RAM READ)");

    // PIO1 - Address Readers
    //
    // SM1 - Address Reader (RAM WRITE)
    //
    // Wait for Data read handler to trigger via IRQ - this indicates /CE and
    // /W went low.
    //
    // Loop reading the address until /W goes high.
    //
    // When /W goes high, push the last read address to the RX FIFO.  This
    // triggers the WRITE DMA chain.
    //
    // The data reader SM is triggered at the same time (actually one cycle
    // later), runs independently , and similarly waits for /W to go high.  As
    // they are both started at around the same time, and take roughly the same
    // time to loop, the data to write should be in the WRITE DMA chain by the
    // time the DMA gets the address and writes the byte.
    PIO_SET_SM(1);

    // Preload high 16 bits of RAM table address to X - done via TX FIFO
    // before starting as SET(X) only supports 5 bits.

    // (SM does not start here.). Push combined RAM table address and lower
    // order address bits when /W goes high.
    PIO_LABEL_NEW(addr_write_valid);
    PIO_ADD_INSTR(PUSH_BLOCK);

    // Wait for address reader IRQ from Data read handler
    PIO_START();
    PIO_ADD_INSTR(WAIT_IRQ_HIGH_PREV(3));

    // Pull high bits from X
    PIO_WRAP_BOTTOM();
    PIO_ADD_INSTR(IN_X(ram_table_num_addr_bits));

    // Read address lines.
    PIO_ADD_INSTR(IN_PINS(config->num_addr_pins));

    // Jump when /W goes high
    PIO_WRAP_TOP();
    PIO_ADD_INSTR(JMP_PIN(PIO_LABEL(addr_write_valid)));

    // SM configuration
    PIO_SM_CLKDIV_SET(
        config->addr_reader_write_clkdiv_int,
        config->addr_reader_write_clkdiv_frac
    );
    PIO_SM_EXECCTRL_SET(
        PIO_JMP_PIN(config->write_pin)
    );
    PIO_SM_SHIFTCTRL_SET(
        PIO_IN_COUNT(config->num_addr_pins) |
        PIO_IN_SHIFTDIR_L |
        PIO_OUT_SHIFTDIR_L
    );
    PIO_SM_PINCTRL_SET(
        PIO_IN_BASE(config->addr_base_pin)
    );

    // Preload the X register to the high bits of the RAM table address
    PIO_TXF = ram_table_high_bits;
    PIO_SM_EXEC_INSTR(PULL_BLOCK);
    PIO_SM_EXEC_INSTR(MOV_X_OSR);

    // Jump to start and log
    PIO_SM_JMP_TO_START();
    PIO_LOG_SM("Address Reader (RAM WRITE)");

    //
    // PIO 0 - End of block
    //
    PIO_END_BLOCK();

    // PIO2 Programs
    //
    // Data Handlers
    PIO_SET_BLOCK(2);

    // PIO2 - Data Handlers
    //
    // SM0 - Data Input/Output handler
    //
    // Start by setting data pins to inputs
    PIO_SET_SM(0);
    PIO_LABEL_NEW(data_io_write_enabled);

    // Set data pins to inputs
    PIO_ADD_INSTR(MOV_PINDIRS_NULL);

    // Test for /CE and /OE active
    PIO_WRAP_BOTTOM();
    PIO_ADD_INSTR(MOV_X_PINS);
    PIO_ADD_INSTR(JMP_X_DEC(PIO_START_LABEL()));    // /CE or /OE inactive.  Have to jump
                                                    // to start and set pins to inputs cos
                                                    // this part of the loop is also used
                                                    // when pins may already be outputs.

    // /CE and /OE low - both active.  Check /W state next
    PIO_LABEL_NEW_OFFSET(data_io_set_outputs, 2);           // Point to set data pins as outputs
    PIO_ADD_INSTR(JMP_PIN(PIO_LABEL(data_io_set_outputs))); // /W disabled, do enable
    PIO_ADD_INSTR(JMP(PIO_LABEL(data_io_write_enabled)));   // /W enabled, don't enable
    PIO_WRAP_TOP();
    PIO_ADD_INSTR(MOV_PINDIRS_NOT_NULL);                    // Set data pins to outputs

    // Configure SM
    PIO_SM_CLKDIV_SET(
        config->data_io_clkdiv_int,
        config->data_io_clkdiv_frac
    );
    PIO_SM_EXECCTRL_SET(
        PIO_JMP_PIN(config->write_pin)
    );
    PIO_SM_SHIFTCTRL_SET(
        PIO_IN_COUNT(config->num_read_cs_pins) |    // /OE amd /CE
        PIO_IN_SHIFTDIR_L                           // Direction doesn't matter
    );
    PIO_SM_PINCTRL_SET(
        PIO_IN_BASE(config->read_cs_base_pin) |     // /OE and /CE
        PIO_OUT_COUNT(config->num_data_pins) |
        PIO_OUT_BASE(config->data_base_pin)
    );

    // Jump to start and log
    PIO_SM_JMP_TO_START();
    PIO_LOG_SM("Data IO Handler");

    //
    // PIO2 - Data Handlers
    //
    // SM1 - Data output (RAM READ)
    //
    // Just waits until 8 bits are made available by the READ DMA chain, then
    // writes them to the data pin outputs (whether they are set to outputs
    // or not).
    PIO_SET_SM(1);
    PIO_ADD_INSTR(OUT_PINS(config->num_data_pins)); // Autopull, blocks until all bits available

    PIO_SM_CLKDIV_SET(
        config->data_out_clkdiv_int,
        config->data_out_clkdiv_frac
    );
    PIO_SM_EXECCTRL_SET(0);
    PIO_SM_SHIFTCTRL_SET(
        PIO_OUT_SHIFTDIR_R |    // Writes LSB of OSR
        PIO_AUTOPULL |          // Auto pull when we hit threshold
        PIO_PULL_THRESH(config->num_data_pins)  // Pull when we have all data bits
    );
    PIO_SM_PINCTRL_SET(
        PIO_OUT_COUNT(config->num_data_pins) |
        PIO_OUT_BASE(config->data_base_pin)
    );

    // Jump to start and log
    PIO_SM_JMP_TO_START();
    PIO_LOG_SM("Data Reader (RAM READ)");

    // PIO2 - Data Handlers
    //
    // SM2 - Data input (RAM WRITE)
    PIO_SET_SM(2);
    PIO_LABEL_NEW(data_in_valid);
    PIO_ADD_INSTR(PUSH_BLOCK);              // Push data to RX FIFO for DMA
    PIO_START();
    PIO_ADD_INSTR(WAIT_IRQ_HIGH_NEXT(3));   // Wait for RAM WRITE IRQ
    PIO_WRAP_BOTTOM();
    PIO_ADD_INSTR(NOP);                     // Synchronise with address reader which takes 2 cycles to read
    PIO_ADD_INSTR(MOV_ISR_PINS);            // Read at same time as address pins
    PIO_WRAP_TOP();
    PIO_ADD_INSTR(JMP_PIN(PIO_LABEL(data_in_valid)));   // Jump when /W goes high 

    PIO_SM_CLKDIV_SET(
        config->data_in_clkdiv_int,
        config->data_in_clkdiv_frac
    );
    PIO_SM_EXECCTRL_SET(
        PIO_JMP_PIN(config->write_pin)
    );
    PIO_SM_SHIFTCTRL_SET(
        PIO_IN_COUNT(config->num_data_pins) |
        PIO_IN_SHIFTDIR_L
    );
    PIO_SM_PINCTRL_SET(
        PIO_IN_BASE(config->data_base_pin)
    );

    // Jump to start and log
    PIO_SM_JMP_TO_START();
    PIO_LOG_SM("Data Reader (RAM WRITE)");

    //
    // PIO 2 - End of block
    //
    PIO_END_BLOCK();
}

// Setup DMA channels for RAM serving
//
// See `dma.c` for notes on RP2350 DMA usage.
static void pioram_setup_dma(pioram_config_t *config) {
    volatile dma_ch_reg_t *dma_reg;
    
    //
    // READ Chain DMAs
    //
    
    // DMA0 - Address Forwarder (READ)
    dma_reg = DMA_CH_REG(0);
    dma_reg->read_addr = (uint32_t)&PIO1_SM_RXF(0);         // Read from RAM READ address reader RX FIFO
    dma_reg->write_addr = (uint32_t)&DMA_CH_READ_ADDR_TRIG(1);  // Write to DMA1 to re-arm it
    dma_reg->transfer_count = 0xffffffff;                   // Re-arm self
    dma_reg->ctrl_trig =
        DMA_CTRL_TRIG_EN |                                  // Enable DMA
        DMA_CTRL_TRIG_IRQ_QUIET |                           // No IRQs
        DMA_CTRL_TRIG_TREQ_SEL(DREQ_PIO_X_SM_Y_RX(1, 0)) |  // Triggered by RAM READ address reader RX FIFO
        DMA_CTRL_TRIG_DATA_SIZE_32BIT |                     // Read a 32-bit RAM READ target address
        DMA_CTRL_TRIG_CHAIN_TO(0);                          // Disable chaining
    
    // DMA1 - Data Fetcher (READ)
    dma_reg = DMA_CH_REG(1);
    dma_reg->read_addr = config->ram_table_addr;            // Placeholder value, written to by DMA0
    dma_reg->write_addr = (uint32_t)&PIO2_SM_TXF(1);        // Write to RAM READ data writer TX FIFO
    dma_reg->transfer_count = 1;                            // Run once, then require re-arming by DMA0 writing to ADDR_TRIG register
    dma_reg->ctrl_trig =
        DMA_CTRL_TRIG_EN |                                  // Enable DMA
        DMA_CTRL_TRIG_IRQ_QUIET |                           // No IRQs
        DMA_CTRL_TRIG_TREQ_SEL(DMA_CTRL_TRIG_TREQ_PERM) |   // Triggered by arming
        DMA_CTRL_TRIG_DATA_SIZE_8BIT |                      // Write 8-bit RAM READ data
        DMA_CTRL_TRIG_CHAIN_TO(0);                          // Disable chaining
    
    //
    // WRITE Chain DMAs
    //
    
    // DMA2 - Address Forwarder (WRITE)
    dma_reg = DMA_CH_REG(2);
    dma_reg->read_addr = (uint32_t)&PIO1_SM_RXF(1);         // Read from RAM WRITE address reader RX FIFO
    dma_reg->write_addr = (uint32_t)&DMA_CH_WRITE_ADDR_TRIG(3);  // Trigger DMA3 to store the data byte
    dma_reg->transfer_count = 0xffffffff;                   // Re-arm self
    dma_reg->ctrl_trig =
        DMA_CTRL_TRIG_EN |                                  // Enable DMA
        DMA_CTRL_TRIG_IRQ_QUIET |                           // No IRQs
        DMA_CTRL_TRIG_PRIORITY_HIGH |                       // High priority
        DMA_CTRL_TRIG_TREQ_SEL(DREQ_PIO_X_SM_Y_RX(1, 1)) |  // Triggered by RAM WRITE address reader RX FIFO
        DMA_CTRL_TRIG_DATA_SIZE_32BIT |                     // Read a 32-bit RAM WRITE target address
        DMA_CTRL_TRIG_CHAIN_TO(2);                          // Disable chaining
    
    // DMA3 - Data Writer (WRITE)
    dma_reg = DMA_CH_REG(3);
    dma_reg->read_addr = (uint32_t)&PIO2_SM_RXF(2);         // Read from RAM WRITE data reader RX FIFO
    dma_reg->write_addr = config->ram_table_addr;           // Placeholder, gets overwritten by DMA2
    dma_reg->transfer_count = 1;
    dma_reg->ctrl_trig =
        DMA_CTRL_TRIG_EN |                                  // Enable DMA
        DMA_CTRL_TRIG_IRQ_QUIET |                           // No IRQs
        DMA_CTRL_TRIG_PRIORITY_HIGH |                       // High priority
        DMA_CTRL_TRIG_DATA_SIZE_8BIT |                      // Store 8-bit RAM WRITE data
        DMA_CTRL_TRIG_TREQ_SEL(DMA_CTRL_TRIG_TREQ_PERM) |   // Triggered by arming
        DMA_CTRL_TRIG_CHAIN_TO(3);                          // Disable chaining
    
    // Set DMA high priority (over CPU access).  It would be possible 
    BUSCTRL_BUS_PRIORITY |=
        BUSCTRL_BUS_PRIORITY_DMA_R_BIT |
        BUSCTRL_BUS_PRIORITY_DMA_W_BIT;
}

// Set GPIOs to PIO function for RAM serving
static void pioram_set_gpio_func(pioram_config_t *config) {
    (void)config;

    // CS pins - not required, as always inputs, and all PIOs can access inputs
    // all the time
    // GPIO_CTRL(10) = GPIO_CTRL_FUNC_PIO2; // /OE
    // GPIO_CTRL(11) = GPIO_CTRL_FUNC_PIO2; // /CE
    // GPIO_CTRL(12) = GPIO_CTRL_FUNC_PIO2; // /W

    // Address pins - not required, as always inputs
    // for (int ii = 13; ii <= 23; ii++) {
    //     GPIO_CTRL(ii) = GPIO_CTRL_FUNC_PIO1;
    // }

    // Data pins
    for (int ii = 0; ii < 8; ii++) {
        GPIO_CTRL(ii) = GPIO_CTRL_FUNC_PIO2;
    }
}

// Start all PIO state machines
static void pioram_start_pios(void) {
    PIO_ENABLE_SM(0, 0x1);  // Enable SM0
    PIO_ENABLE_SM(1, 0x3);  // Enable SM0 and
    PIO_ENABLE_SM(2, 0x7);  // Enable SM0, SM1, and SM2
    DEBUG("RAM PIOs started");
}

// Extern RAM/ROM image start symbol from linker script.  Used because,
// currently main() does not provide the correct address to pioram().
extern uint32_t _ram_rom_image_start[];

// Top-level RAM serving function
void pioram(
    const sdrr_info_t *info,
    uint32_t ram_table_addr
) {
    (void)info;

    DEBUG("%s", log_divider);

    ram_table_addr = (uint32_t)_ram_rom_image_start;

#if defined(DEBUG_BUILD) && (DEBUG_BUILD == 1)
    // Clear 64KB RAM table
    uint8_t *ram_table_ptr = (uint8_t *)ram_table_addr;
    for (int ii = 0; ii < 65536; ii++) {
        ram_table_ptr[ii] = 0x03;
    }
#endif // DEBUG_BUILD

    pioram_config_t config = {
        .read_cs_base_pin = 10,     // /OE + /CE, fire-24-d
        .num_read_cs_pins = 2,
        .write_cs_base_pin = 11,    // /CE + /W, fire-24-d
        .num_write_cs_pins = 2,
        .write_pin = 12,            // /W pin, fire-24-d
        .data_base_pin = 0,         // fire-24-d
        .num_data_pins = 8,
        .addr_base_pin = 13,        // fire-24-d
        .num_addr_pins = 11,        // 6116 has A0-A10
        .ram_table_addr = ram_table_addr,
        .data_read_handler_clkdiv_int = 1,
        .data_read_handler_clkdiv_frac = 0,
        .addr_reader_read_clkdiv_int = 1,
        .addr_reader_read_clkdiv_frac = 0,
        .addr_reader_write_clkdiv_int = 1,
        .addr_reader_write_clkdiv_frac = 0,
        .data_io_clkdiv_int = 1,
        .data_io_clkdiv_frac = 0,
        .data_out_clkdiv_int = 1,
        .data_out_clkdiv_frac = 0,
        .data_in_clkdiv_int = 1,
        .data_in_clkdiv_frac = 0,
    };
    
    // Bring PIO0, PIO1, PIO2 and DMA out of reset
    RESET_RESET &= ~(RESET_PIO0 | RESET_PIO1 | RESET_PIO2 | RESET_DMA);
    while (!(RESET_DONE & (RESET_PIO0 | RESET_PIO1 | RESET_PIO2 | RESET_DMA)));
    
    // Setup DMA channels
    pioram_setup_dma(&config);
    
    // Configure GPIOs
    pioram_set_gpio_func(&config);

    // Load PIO programs
    pioram_load_programs(&config);
    
    // Start PIOs
    pioram_start_pios();
    DEBUG("PIO RAM serving started");
    DEBUG("%s", log_divider);

#define PIO_DEBUG_LOOP 1
#if defined(PIO_DEBUG_LOOP)
    // Output PIO and DMA debug information periodically
    uint32_t last_read_addr = 0xFFFFFFFF;
    uint32_t last_write_addr = 0xFFFFFFFF;
    uint8_t read_addr_still_unchanged = 0;
    uint8_t write_addr_still_unchanged = 0;
    while (1) {
        // See if any PIO FIFOs are full
        uint32_t volatile pio_fstats[3] = {
            PIO0_FSTAT,
            PIO1_FSTAT,
            PIO2_FSTAT
        };
        for (int ii = 0; ii < 3; ii++) {
            uint32_t pio_fstat = pio_fstats[ii];
            for (int jj = 0; jj < 4; jj++) {
                uint8_t rxfull_bit = 0 + jj;
                uint8_t txfull_bit = 16 + jj;
                if (pio_fstat & (1 << rxfull_bit)) {
                    DEBUG("!!! PIO%d SM%d RXFULL set", ii, jj);
                }
                if (pio_fstat & (1 << txfull_bit)) {
                    DEBUG("!!! PIO%d SM%d TXFULL set", ii, jj);
                }
            }
        }

        // Check the DMA read/write RAM table addresses are changing.
        // The odd log here is acceptable - but constant unchanging read or
        // write addresses suggest a problem (for example, host has crashed).
        // As such we only log if at least the last three checks have been
        // the same.
        volatile dma_ch_reg_t *dma1 = DMA_CH_REG(1);
        volatile dma_ch_reg_t *dma3 = DMA_CH_REG(3);
        uint32_t new_read_addr = dma1->read_addr;
        uint32_t new_write_addr = dma3->write_addr;
        if (new_read_addr == last_read_addr) {
            if (read_addr_still_unchanged > 1) {
                DEBUG("!!! RAM READ address unchanged: 0x%08X", new_read_addr);
            }
            read_addr_still_unchanged++;
        } else {
            read_addr_still_unchanged = 0;
        }
        if (new_write_addr == last_write_addr) {
            if (write_addr_still_unchanged > 1) {
                DEBUG("!!! RAM WRITE address unchanged: 0x%08X", new_write_addr);
            }
            write_addr_still_unchanged++;
        } else {
            write_addr_still_unchanged = 0;
        }
        last_read_addr = new_read_addr;
        last_write_addr = new_write_addr;

        // Delay before next check
        #define PIO_DEBUG_LOOP_DELAY 1000000
        for (volatile int i = 0; i < PIO_DEBUG_LOOP_DELAY; i++);
    }
#endif // PIO_DEBUG_LOOP

    // Low power loop
    while (1) {
        __asm volatile("wfi");
    }
}


#endif // RP235X
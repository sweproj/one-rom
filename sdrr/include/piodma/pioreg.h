// Copyright (C) 2026 Piers Finlayson <piers@piers.rocks>
//
// MIT License

// RP2350 PIO register definitions

#ifndef PIOREG_H
#define PIOREG_H

#include <stdint.h>

// Base register addresses
#define DMA_BASE            (0x50000000)
#define PIO0_BASE           (0x50200000)
#define PIO1_BASE           (0x50300000)
#define PIO2_BASE           (0x50400000)

// PIO register offsets
#define PIO_CTRL_OFFSET         (0x00)
#define PIO_FSTAT_OFFSET        (0x04)
#define PIO_FDEBUG_OFFSET       (0x08)
#define PIO_FLEVEL_OFFSET       (0x0C)
#define PIO_TXF_OFFSET          (0x10)
#define PIO_RXF_OFFSET          (0x20)
#define PIO_IRQ_OFFSET          (0x30)
#define PIO_IRQ_FORCE_OFFSET    (0x34)
#define PIO_INPUT_SYNC_BYPASS_OFFSET (0x38)
#define PIO_INSTR_MEM_OFFSET    (0x48)
#define PIO_SM_REG_OFFSET       (0xC8)
#define PIO_SM_RXF_OFFSET       (0x128)
#define PIO_SM_TXF_OFFSET       (0x138)

/// Macros for accessing PIO control registers
#define PIO0_CTRL       (*(volatile uint32_t *)(PIO0_BASE + PIO_CTRL_OFFSET))
#define PIO1_CTRL       (*(volatile uint32_t *)(PIO1_BASE + PIO_CTRL_OFFSET))
#define PIO2_CTRL       (*(volatile uint32_t *)(PIO2_BASE + PIO_CTRL_OFFSET))
#define PIO0_FSTAT      (*(volatile uint32_t *)(PIO0_BASE + PIO_FSTAT_OFFSET))
#define PIO1_FSTAT      (*(volatile uint32_t *)(PIO1_BASE + PIO_FSTAT_OFFSET))
#define PIO2_FSTAT      (*(volatile uint32_t *)(PIO2_BASE + PIO_FSTAT_OFFSET))
#define PIO0_FLEVEL     (*(volatile uint32_t *)(PIO0_BASE + PIO_FLEVEL_OFFSET))
#define PIO1_FLEVEL     (*(volatile uint32_t *)(PIO1_BASE + PIO_FLEVEL_OFFSET))
#define PIO2_FLEVEL     (*(volatile uint32_t *)(PIO2_BASE + PIO_FLEVEL_OFFSET))
#define PIO0_SM_TXF(X)  (*(volatile uint32_t *)(PIO0_BASE + PIO_TXF_OFFSET + ((X) * 0x04)))
#define PIO1_SM_TXF(X)  (*(volatile uint32_t *)(PIO1_BASE + PIO_TXF_OFFSET + ((X) * 0x04)))
#define PIO2_SM_TXF(X)  (*(volatile uint32_t *)(PIO2_BASE + PIO_TXF_OFFSET + ((X) * 0x04)))
#define PIO0_SM_RXF(X)  (*(volatile uint32_t *)(PIO0_BASE + PIO_RXF_OFFSET + ((X) * 0x04)))
#define PIO1_SM_RXF(X)  (*(volatile uint32_t *)(PIO1_BASE + PIO_RXF_OFFSET + ((X) * 0x04)))
#define PIO2_SM_RXF(X)  (*(volatile uint32_t *)(PIO2_BASE + PIO_RXF_OFFSET + ((X) * 0x04)))
#define PIO0_IRQ        (*(volatile uint32_t *)(PIO0_BASE + PIO_IRQ_OFFSET))
#define PIO1_IRQ        (*(volatile uint32_t *)(PIO1_BASE + PIO_IRQ_OFFSET))
#define PIO2_IRQ        (*(volatile uint32_t *)(PIO2_BASE + PIO_IRQ_OFFSET))
#define PIO0_IRQ_FORCE  (*(volatile uint32_t *)(PIO0_BASE + PIO_IRQ_FORCE_OFFSET))
#define PIO1_IRQ_FORCE  (*(volatile uint32_t *)(PIO1_BASE + PIO_IRQ_FORCE_OFFSET))
#define PIO2_IRQ_FORCE  (*(volatile uint32_t *)(PIO2_BASE + PIO_IRQ_FORCE_OFFSET))
#define PIO0_INPUT_SYNC_BYPASS  (*(volatile uint32_t *)(PIO0_BASE + PIO_INPUT_SYNC_BYPASS_OFFSET))
#define PIO1_INPUT_SYNC_BYPASS  (*(volatile uint32_t *)(PIO1_BASE + PIO_INPUT_SYNC_BYPASS_OFFSET))
#define PIO2_INPUT_SYNC_BYPASS  (*(volatile uint32_t *)(PIO2_BASE + PIO_INPUT_SYNC_BYPASS_OFFSET))

// Macros for PIO control registers
#define PIO_CTRL_SM_ENABLE(X)      ((X & 0xf) << 0)
#define PIO0_CTRL_SM_ENABLE(X)     PIO0_CTRL = PIO_CTRL_SM_ENABLE(X)
#define PIO1_CTRL_SM_ENABLE(X)     PIO1_CTRL = PIO_CTRL_SM_ENABLE(X)
#define PIO2_CTRL_SM_ENABLE(X)     PIO2_CTRL = PIO_CTRL_SM_ENABLE(X)

// Macros for PIO FSTAT registers
#define PIO_FSTAT_SMX_RX_EMPTY_BIT(X)       (1 << (X + 8))
#define PIO0_FSTAT_SMX_RX_EMPTY(X)          (PIO_FSTAT_SMX_RX_EMPTY_BIT(X) & PIO0_FSTAT)

// Macros for filling PIO instruction memory
#define PIO0_INSTR_MEM(X)       (*(volatile uint32_t *)(PIO0_BASE + PIO_INSTR_MEM_OFFSET + ((X) * 4)))
#define PIO1_INSTR_MEM(X)       (*(volatile uint32_t *)(PIO1_BASE + PIO_INSTR_MEM_OFFSET + ((X) * 4)))
#define PIO2_INSTR_MEM(X)       (*(volatile uint32_t *)(PIO2_BASE + PIO_INSTR_MEM_OFFSET + ((X) * 4)))

typedef struct pio_sm_reg {
    uint32_t clkdiv;
    uint32_t execctrl;
    uint32_t shiftctrl;
    uint32_t addr;
    uint32_t instr;
    uint32_t pinctrl; 
} pio_sm_reg_t;

// Macros for accessing PIO state machine registers
#define PIO0_SM_REG(X)      ((volatile pio_sm_reg_t *)(PIO0_BASE + PIO_SM_REG_OFFSET + ((X) * 0x18)))
#define PIO1_SM_REG(X)      ((volatile pio_sm_reg_t *)(PIO1_BASE + PIO_SM_REG_OFFSET + ((X) * 0x18)))
#define PIO2_SM_REG(X)      ((volatile pio_sm_reg_t *)(PIO2_BASE + PIO_SM_REG_OFFSET + ((X) * 0x18)))

// Macros to build PIO SM registers

// CLKDIV
#define PIO_CLKDIV(INT, FRAC)           (((INT) & 0xFFFF) << 16 | ((FRAC) & 0xFF) << 8)
#define PIO_CLKDIV_INT_FROM_REG(REG)    (((REG) >> 16) & 0xFFFF)
#define PIO_CLKDIV_FRAC_FROM_REG(REG)   (((REG) >> 8) & 0xFF)

// EXECCTRL
#define PIO_WRAP_BOTTOM_AS_REG(X)   (((X) & 0x1F) << 7)
#define PIO_WRAP_TOP_AS_REG(X)      (((X) & 0x1F) << 12)
#define PIO_JMP_PIN(X)          (((X) & 0x1F) << 24)
#define PIO_WRAP_TOP_FROM_REG(REG)    (((REG) >> 12) & 0x1F)
#define PIO_WRAP_BOTTOM_FROM_REG(REG) (((REG) >> 7) & 0x1F)

// SHIFTCTRL
#define PIO_IN_COUNT(X)         (((X) & 0x1F) << 0)
#define PIO_AUTOPUSH            (1 << 16)
#define PIO_AUTOPULL            (1 << 17)
#define PIO_IN_SHIFTDIR_R       (1 << 18)
#define PIO_IN_SHIFTDIR_L       (0 << 18)
#define PIO_OUT_SHIFTDIR_R      (1 << 19)
#define PIO_OUT_SHIFTDIR_L      (0 << 19)
#define PIO_PUSH_THRESH(X)      (((X) & 0x1F) << 20)
#define PIO_PULL_THRESH(X)      (((X) & 0x1F) << 25)

// PINCTRL
#define PIO_OUT_BASE(X)         (((X) & 0x1F) << 0)
#define PIO_SET_BASE(X)         (((X) & 0x1F) << 5)
#define PIO_SIDE_SET_BASE(X)    (((X) & 0x1F) << 10)
#define PIO_IN_BASE(X)          (((X) & 0x1F) << 15)
#define PIO_OUT_COUNT(X)        (((X) & 0x3F) << 20)
#define PIO_SET_COUNT(X)        (((X) & 0x07) << 26)
#define PIO_SIDE_SET_COUNT(X)   (((X) & 0x07) << 29)

// Macro to access a PIO state machine RX FIFO entry
#define PIO0_SM_X_RXF_Y(X, Y)   (*(volatile uint32_t *)(PIO0_BASE + PIO_SM_RXF_OFFSET + ((X) * 0x10) + ((Y) * 4)))
#define PIO1_SM_X_RXF_Y(X, Y)   (*(volatile uint32_t *)(PIO1_BASE + PIO_SM_RXF_OFFSET + ((X) * 0x10) + ((Y) * 4)))
#define PIO2_SM_X_RXF_Y(X, Y)   (*(volatile uint32_t *)(PIO2_BASE + PIO_SM_RXF_OFFSET + ((X) * 0x10) + ((Y) * 4)))

// Macros to construct DREQ values
#define DREQ_PIO_X_SM_Y_TX(X, Y)      (0 + (X * 8) + Y)
#define DREQ_PIO_X_SM_Y_RX(X, Y)      (4 + (X * 8) + Y)

#endif // PIOREG_H
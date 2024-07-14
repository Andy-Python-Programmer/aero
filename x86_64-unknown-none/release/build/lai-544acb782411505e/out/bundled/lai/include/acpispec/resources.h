/*
 * Lightweight AML Interpreter
 * Copyright (C) 2018-2023 The lai authors
 */

#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Device _STA object
#define ACPI_STA_PRESENT 0x01
#define ACPI_STA_ENABLED 0x02
#define ACPI_STA_VISIBLE 0x04
#define ACPI_STA_FUNCTION 0x08
#define ACPI_STA_BATTERY 0x10

// Parsing Resource Templates
#define ACPI_RESOURCE_MEMORY 1
#define ACPI_RESOURCE_IO 2
#define ACPI_RESOURCE_IRQ 3

// IRQ Flags
#define ACPI_SMALL_IRQ_EDGE_TRIGGERED 0x01
#define ACPI_SMALL_IRQ_ACTIVE_LOW 0x08
#define ACPI_SMALL_IRQ_SHARED 0x10
#define ACPI_SMALL_IRQ_WAKE 0x20
#define ACPI_EXTENDED_IRQ_EDGE_TRIGGERED 0x02
#define ACPI_EXTENDED_IRQ_ACTIVE_LOW 0x04
#define ACPI_EXTENDED_IRQ_SHARED 0x08
#define ACPI_EXTENDED_IRQ_WAKE 0x10

typedef struct acpi_resource_t {
    uint8_t type;

    uint64_t base; // valid for everything

    uint64_t length; // valid for I/O and MMIO

    uint8_t address_space; // these are valid --
    uint8_t bit_width; // -- only for --
    uint8_t bit_offset; // -- generic registers

    uint8_t irq_flags; // valid for IRQs
} acpi_resource_t;

typedef struct acpi_small_irq_t {
    uint8_t id;
    uint16_t irq_mask;
    uint8_t config;
} __attribute__((packed)) acpi_small_irq_t;

typedef struct acpi_large_irq_t {
    uint8_t id;
    uint16_t size;
    uint8_t config;
    uint8_t length;
    uint32_t irq;
} __attribute__((packed)) acpi_large_irq_t;

#ifdef __cplusplus
}
#endif

/*
 * Lightweight AML Interpreter
 * Copyright (C) 2018-2023 The lai authors
 */

#pragma once

#include <lai/core.h>

#ifdef __cplusplus
extern "C" {
#endif

__attribute__((deprecated("use lai_resource_view instead"))) size_t
lai_read_resource(lai_nsnode_t *, acpi_resource_t *);

enum lai_resource_type {
    LAI_RESOURCE_NULL,
    LAI_RESOURCE_IRQ,
    LAI_RESOURCE_DMA,
    LAI_RESOURCE_IO,
    LAI_RESOURCE_MEM,
    LAI_RESOURCE_VENDOR,
    LAI_RESOURCE_REGISTER,
};

struct lai_resource_view {
    uint8_t *entry;
    size_t skip_size;
    size_t entry_idx;
    lai_variable_t *crs_var;

    uint64_t base; // MMIO / IO / Generic Addresses
    uint64_t length;
    uint64_t alignment;
    uint8_t flags;

    uint8_t address_space; // Generic Addresses
    uint8_t bit_width;
    uint8_t bit_offset;

    uint32_t gsi; // Large IRQs
};

#define LAI_RESOURCE_VIEW_INITIALIZER(crs)                                                         \
    {                                                                                              \
        .entry = (uint8_t *)lai_exec_buffer_access(crs), .skip_size = 0, .entry_idx = 0,           \
        .crs_var = crs, .base = 0, .length = 0, .alignment = 0, .flags = 0, .address_space = 0,    \
        .bit_width = 0, .bit_offset = 0, .gsi = 0                                                  \
    }

inline static void lai_initialize_resource_view(struct lai_resource_view *view,
                                                lai_variable_t *crs) {
    *view = (struct lai_resource_view)LAI_RESOURCE_VIEW_INITIALIZER(crs);
}

lai_api_error_t lai_resource_iterate(struct lai_resource_view *);

enum lai_resource_type lai_resource_get_type(struct lai_resource_view *);
int lai_resource_irq_is_level_triggered(struct lai_resource_view *);
int lai_resource_irq_is_active_low(struct lai_resource_view *);
lai_api_error_t lai_resource_next_irq(struct lai_resource_view *iterator);

#ifdef __cplusplus
}
#endif

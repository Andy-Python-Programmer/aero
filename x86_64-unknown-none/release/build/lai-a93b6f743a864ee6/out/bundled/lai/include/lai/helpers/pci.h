/*
 * Lightweight AML Interpreter
 * Copyright (C) 2018-2023 The lai authors
 */

#pragma once

#include <lai/core.h>

#ifdef __cplusplus
extern "C" {
#endif

__attribute__((deprecated("use lai_pci_route_pin instead"))) int
lai_pci_route(acpi_resource_t *, uint16_t, uint8_t, uint8_t, uint8_t);
lai_api_error_t lai_pci_route_pin(acpi_resource_t *, uint16_t, uint8_t, uint8_t, uint8_t, uint8_t);

lai_nsnode_t *lai_pci_find_device(lai_nsnode_t *, uint8_t, uint8_t, lai_state_t *);
lai_nsnode_t *lai_pci_find_bus(uint16_t, uint8_t, lai_state_t *);

struct lai_prt_iterator {
    size_t i;
    lai_variable_t *prt;

    int slot, function;
    uint8_t pin;
    lai_nsnode_t *link;
    size_t resource_idx;
    uint32_t gsi;
    uint8_t level_triggered;
    uint8_t active_low;
};

#define LAI_PRT_ITERATOR_INITIALIZER(prt)                                                          \
    { 0, prt, 0, 0, 0, NULL, 0, 0, 0, 0 }

inline void lai_initialize_prt_iterator(struct lai_prt_iterator *iter, lai_variable_t *prt) {
    *iter = (struct lai_prt_iterator)LAI_PRT_ITERATOR_INITIALIZER(prt);
}

lai_api_error_t lai_pci_parse_prt(struct lai_prt_iterator *iter);

#ifdef __cplusplus
}
#endif

/*
 * Lightweight AML Interpreter
 * Copyright (C) 2018-2023 The lai authors
 */

#pragma once

#include <lai/core.h>

#ifdef __cplusplus
extern "C" {
#endif

struct lai_rsdp_info {
    // ACPI version (1 or 2).
    int acpi_version;
    // Physical addresses of RSDP and RSDT.
    uintptr_t rsdp_address;
    uintptr_t rsdt_address;
    uintptr_t xsdt_address;
};

lai_api_error_t lai_bios_detect_rsdp_within(uintptr_t base, size_t length,
                                            struct lai_rsdp_info *info);

lai_api_error_t lai_bios_detect_rsdp(struct lai_rsdp_info *info);

#ifdef __cplusplus
}
#endif

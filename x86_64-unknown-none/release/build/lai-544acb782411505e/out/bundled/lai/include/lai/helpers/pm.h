/*
 * Lightweight AML Interpreter
 * Copyright (C) 2018-2023 The lai authors
 */

#pragma once

#include <lai/core.h>

#ifdef __cplusplus
extern "C" {
#endif

lai_api_error_t lai_enter_sleep(uint8_t);
lai_api_error_t lai_acpi_reset();

#ifdef __cplusplus
}
#endif

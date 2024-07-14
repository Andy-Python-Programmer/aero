/*
 * Lightweight AML Interpreter
 * Copyright (C) 2018-2023 The lai authors
 */

#pragma once

#include <lai/core.h>

#ifdef __cplusplus
extern "C" {
#endif

uint32_t lai_read_pm_timer_value();
lai_api_error_t lai_start_pm_timer();
lai_api_error_t lai_stop_pm_timer();
lai_api_error_t lai_busy_wait_pm_timer(uint64_t);

#ifdef __cplusplus
}
#endif

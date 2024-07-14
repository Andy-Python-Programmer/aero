/*
 * Lightweight AML Interpreter
 * Copyright (C) 2018-2023 The lai authors
 */

#pragma once

#ifdef __cplusplus
extern "C" {
#endif

// Don't forget to add new errors to lai_api_error_to_string in error.c
typedef enum lai_api_error {
    LAI_ERROR_NONE,
    LAI_ERROR_OUT_OF_MEMORY,
    LAI_ERROR_TYPE_MISMATCH,
    LAI_ERROR_NO_SUCH_NODE,
    LAI_ERROR_OUT_OF_BOUNDS,
    LAI_ERROR_EXECUTION_FAILURE,

    LAI_ERROR_ILLEGAL_ARGUMENTS,

    /* Evaluating external inputs (e.g., nodes of the ACPI namespace) returned an unexpected result.
     * Unlike LAI_ERROR_EXECUTION_FAILURE, this error does not indicate that
     * execution of AML failed; instead, the resulting object fails to satisfy some
     * expectation (e.g., it is of the wrong type, has an unexpected size, or consists of
     * unexpected contents) */
    LAI_ERROR_UNEXPECTED_RESULT,

    // Error given when end of iterator is reached, nothing to worry about
    LAI_ERROR_END_REACHED,

    LAI_ERROR_UNSUPPORTED,
} lai_api_error_t;

#ifdef __cplusplus
}
#endif

/*
 * Lightweight AML Interpreter
 * Copyright (C) 2018-2023 The lai authors
 */

#pragma once

#include <stddef.h>

#define LAI_CONTAINER_OF(inner_ptr, outer_type, outer_member)                                      \
    ({                                                                                             \
        const __typeof__(((outer_type *)0)->outer_member) *__lai_null_container_member             \
            = (inner_ptr);                                                                         \
        (outer_type *)((char *)__lai_null_container_member - offsetof(outer_type, outer_member));  \
    })

#define LAI_SIZEOF_ARRAY(array) (sizeof(array) / sizeof((array)[0]))

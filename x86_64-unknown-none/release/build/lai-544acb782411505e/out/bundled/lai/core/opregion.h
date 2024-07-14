/*
 * Lightweight AML Interpreter
 * Copyright (C) 2018-2023 The lai authors
 */

#pragma once

#include <lai/core.h>

void lai_read_opregion(lai_variable_t *, lai_nsnode_t *);
void lai_write_opregion(lai_nsnode_t *, lai_variable_t *);

void lai_write_field(lai_nsnode_t *, lai_variable_t *);
void lai_read_field(lai_variable_t *, lai_nsnode_t *);

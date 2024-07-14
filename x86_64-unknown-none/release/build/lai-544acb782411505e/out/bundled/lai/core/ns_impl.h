/*
 * Lightweight AML Interpreter
 * Copyright (C) 2018-2023 The lai authors
 */

// Internal header file. Do not use outside of LAI.

#pragma once

#include <lai/core.h>

// Namespace management.
lai_nsnode_t *lai_create_nsnode(void);
lai_nsnode_t *lai_create_nsnode_or_die(void);
lai_api_error_t lai_install_nsnode(lai_nsnode_t *node);
void lai_uninstall_nsnode(lai_nsnode_t *node);

// Sets the name and parent of a namespace node.
size_t lai_resolve_new_node(lai_nsnode_t *node, lai_nsnode_t *ctx_handle, void *data);

/*
 * Lightweight AML Interpreter
 * Copyright (C) 2018-2023 The lai authors
 */

#pragma once

#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>

#include <acpispec/hw.h>
#include <acpispec/resources.h>
#include <acpispec/tables.h>
#include <lai/error.h>
#include <lai/host.h>
#include <lai/internal-exec.h>
#include <lai/internal-ns.h>
#include <lai/internal-util.h>

#ifdef __cplusplus
extern "C" {
#endif

#define LAI_REVISION 0x20200712

#define ACPI_MAX_RESOURCES 512

// Convert a lai_api_error_t to a human readable string
const char *lai_api_error_to_string(lai_api_error_t);

struct lai_instance {
    lai_nsnode_t *root_node;

    lai_nsnode_t **ns_array;
    size_t ns_size;
    size_t ns_capacity;

    int acpi_revision;
    int trace;
    int is_hw_reduced;

    acpi_fadt_t *fadt;
};

struct lai_instance *lai_current_instance();

void lai_init_state(lai_state_t *);
void lai_finalize_state(lai_state_t *);

#define LAI_CLEANUP_STATE __attribute__((cleanup(lai_finalize_state)))

struct lai_ns_iterator {
    size_t i;
};

struct lai_ns_child_iterator {
    size_t i;
    lai_nsnode_t *parent;
};

#define LAI_NS_ITERATOR_INITIALIZER                                                                \
    { 0 }
#define LAI_NS_CHILD_ITERATOR_INITIALIZER(x)                                                       \
    { 0, x }

static inline void lai_initialize_ns_iterator(struct lai_ns_iterator *iter) {
    *iter = (struct lai_ns_iterator)LAI_NS_ITERATOR_INITIALIZER;
}

static inline void lai_initialize_ns_child_iterator(struct lai_ns_child_iterator *iter,
                                                    lai_nsnode_t *parent) {
    *iter = (struct lai_ns_child_iterator)LAI_NS_CHILD_ITERATOR_INITIALIZER(parent);
}

extern volatile uint16_t lai_last_event;

// The remaining of these functions are OS independent!
// ACPI namespace functions
lai_nsnode_t *lai_create_root(void);
void lai_create_namespace(void);
char *lai_stringify_node_path(lai_nsnode_t *);
lai_nsnode_t *lai_resolve_path(lai_nsnode_t *, const char *);
lai_nsnode_t *lai_resolve_search(lai_nsnode_t *, const char *);
lai_nsnode_t *lai_get_device(size_t);
int lai_check_device_pnp_id(lai_nsnode_t *, lai_variable_t *, lai_state_t *);
lai_nsnode_t *lai_enum(char *, size_t);
void lai_eisaid(lai_variable_t *, const char *);
lai_nsnode_t *lai_ns_iterate(struct lai_ns_iterator *);
lai_nsnode_t *lai_ns_child_iterate(struct lai_ns_child_iterator *);

// Namespace functions.

lai_nsnode_t *lai_ns_get_root();
lai_nsnode_t *lai_ns_get_parent(lai_nsnode_t *node);
lai_nsnode_t *lai_ns_get_child(lai_nsnode_t *parent, const char *name);
lai_api_error_t lai_ns_override_notify(lai_nsnode_t *node,
                                       lai_api_error_t (*override)(lai_nsnode_t *, int, void *),
                                       void *userptr);
lai_api_error_t lai_ns_override_opregion(lai_nsnode_t *node,
                                         const struct lai_opregion_override *override,
                                         void *userptr);
enum lai_node_type lai_ns_get_node_type(lai_nsnode_t *node);

uint8_t lai_ns_get_opregion_address_space(lai_nsnode_t *node);

// Access and manipulation of lai_variable_t.

enum lai_object_type {
    LAI_TYPE_NONE,
    LAI_TYPE_INTEGER,
    LAI_TYPE_STRING,
    LAI_TYPE_BUFFER,
    LAI_TYPE_PACKAGE,
    LAI_TYPE_DEVICE,
};

lai_api_error_t lai_create_string(lai_variable_t *, size_t);
lai_api_error_t lai_create_c_string(lai_variable_t *, const char *);
lai_api_error_t lai_create_buffer(lai_variable_t *, size_t);
lai_api_error_t lai_create_pkg(lai_variable_t *, size_t);

enum lai_object_type lai_obj_get_type(lai_variable_t *object);
lai_api_error_t lai_obj_get_integer(lai_variable_t *, uint64_t *);
lai_api_error_t lai_obj_get_pkg(lai_variable_t *, size_t, lai_variable_t *);
lai_api_error_t lai_obj_get_handle(lai_variable_t *, lai_nsnode_t **);

lai_api_error_t lai_obj_resize_string(lai_variable_t *, size_t);
lai_api_error_t lai_obj_resize_buffer(lai_variable_t *, size_t);
lai_api_error_t lai_obj_resize_pkg(lai_variable_t *, size_t);

lai_api_error_t lai_obj_to_buffer(lai_variable_t *, lai_variable_t *);
lai_api_error_t lai_mutate_buffer(lai_variable_t *, lai_variable_t *);
lai_api_error_t lai_obj_to_string(lai_variable_t *, lai_variable_t *, size_t);
lai_api_error_t lai_obj_to_decimal_string(lai_variable_t *, lai_variable_t *);
lai_api_error_t lai_obj_to_hex_string(lai_variable_t *, lai_variable_t *);
lai_api_error_t lai_mutate_string(lai_variable_t *, lai_variable_t *);
lai_api_error_t lai_obj_to_integer(lai_variable_t *, lai_variable_t *);
lai_api_error_t lai_mutate_integer(lai_variable_t *, lai_variable_t *);
lai_api_error_t lai_obj_to_type_string(lai_variable_t *target, lai_nsnode_t *object);
void lai_obj_clone(lai_variable_t *, lai_variable_t *);

int lai_objecttype_ns(lai_nsnode_t *);
int lai_objecttype_obj(lai_variable_t *);

lai_api_error_t lai_obj_exec_match_op(int, lai_variable_t *, lai_variable_t *, int *);

#define LAI_CLEANUP_VAR __attribute__((cleanup(lai_var_finalize)))
#ifdef __cplusplus
#define LAI_VAR_INITIALIZER                                                                        \
    {}
#else
#define LAI_VAR_INITIALIZER                                                                        \
    { 0 }
#endif

static inline void lai_var_initialize(lai_variable_t *var) {
    *var = (lai_variable_t)LAI_VAR_INITIALIZER;
}

void lai_var_finalize(lai_variable_t *);
void lai_var_move(lai_variable_t *, lai_variable_t *);
void lai_var_assign(lai_variable_t *, lai_variable_t *);

// Evaluation of namespace nodes (including control methods).

lai_api_error_t lai_eval_args(lai_variable_t *, lai_nsnode_t *, lai_state_t *, int,
                              lai_variable_t *);
lai_api_error_t lai_eval_largs(lai_variable_t *, lai_nsnode_t *, lai_state_t *, ...);
lai_api_error_t lai_eval_vargs(lai_variable_t *, lai_nsnode_t *, lai_state_t *, va_list);
lai_api_error_t lai_eval(lai_variable_t *, lai_nsnode_t *, lai_state_t *);

// ACPI Control Methods
lai_api_error_t lai_populate(lai_nsnode_t *, struct lai_aml_segment *, lai_state_t *);

// LAI initialization functions
void lai_set_acpi_revision(int);

// LAI debugging functions.

#define LAI_TRACE_OP 1
#define LAI_TRACE_IO 2
#define LAI_TRACE_NS 4

void lai_enable_tracing(int trace);

#ifdef __cplusplus
}
#endif

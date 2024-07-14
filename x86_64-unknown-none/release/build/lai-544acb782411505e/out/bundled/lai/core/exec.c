/*
 * Lightweight AML Interpreter
 * Copyright (C) 2018-2023 The lai authors
 */

#include <lai/core.h>

#include "aml_opcodes.h"
#include "eval.h"
#include "exec_impl.h"
#include "libc.h"
#include "ns_impl.h"
#include "util-list.h"
#include "util-macros.h"

static int debug_stack = 0;

static lai_api_error_t lai_exec_process(lai_state_t *state);
static lai_api_error_t lai_exec_parse(int parse_mode, lai_state_t *state);

// Prepare the interpreter state for a control method call.
// Param: lai_state_t *state - will store method name and arguments
// Param: lai_nsnode_t *method - identifies the control method

void lai_init_state(lai_state_t *state) {
    memset(state, 0, sizeof(lai_state_t));
    state->ctxstack_base = state->small_ctxstack;
    state->blkstack_base = state->small_blkstack;
    state->stack_base = state->small_stack;
    state->opstack_base = state->small_opstack;
    state->ctxstack_capacity = LAI_SMALL_CTXSTACK_SIZE;
    state->blkstack_capacity = LAI_SMALL_BLKSTACK_SIZE;
    state->stack_capacity = LAI_SMALL_STACK_SIZE;
    state->opstack_capacity = LAI_SMALL_OPSTACK_SIZE;
    state->ctxstack_ptr = -1;
    state->blkstack_ptr = -1;
    state->stack_ptr = -1;
}

// Finalize the interpreter state. Frees all memory owned by the state.
void lai_finalize_state(lai_state_t *state) {
    while (state->ctxstack_ptr >= 0)
        lai_exec_pop_ctxstack_back(state);
    while (state->blkstack_ptr >= 0)
        lai_exec_pop_blkstack_back(state);
    while (state->stack_ptr >= 0)
        lai_exec_pop_stack_back(state);
    lai_exec_pop_opstack(state, state->opstack_ptr);

    if (state->ctxstack_base != state->small_ctxstack)
        laihost_free(state->ctxstack_base, state->ctxstack_capacity * sizeof(struct lai_ctxitem));
    if (state->blkstack_base != state->small_blkstack)
        laihost_free(state->blkstack_base, state->blkstack_capacity * sizeof(struct lai_blkitem));
    if (state->stack_base != state->small_stack)
        laihost_free(state->stack_base, state->stack_capacity * sizeof(lai_stackitem_t));
    if (state->opstack_base != state->small_opstack)
        laihost_free(state->opstack_base, state->opstack_capacity * sizeof(struct lai_operand));
}

static lai_api_error_t lai_exec_reduce_node(int opcode, lai_state_t *state,
                                            struct lai_operand *operands,
                                            lai_nsnode_t *ctx_handle) {
    if (lai_current_instance()->trace & LAI_TRACE_OP)
        lai_debug("lai_exec_reduce_node: opcode 0x%02X", opcode);
    switch (opcode) {
        case NAME_OP: {
            lai_variable_t object = {0};
            lai_exec_get_objectref(state, &operands[1], &object);
            LAI_ENSURE(operands[0].tag == LAI_UNRESOLVED_NAME);

            struct lai_amlname amln;
            lai_amlname_parse(&amln, operands[0].unres_aml);

            lai_nsnode_t *node = lai_create_nsnode_or_die();
            node->type = LAI_NAMESPACE_NAME;
            lai_do_resolve_new_node(node, ctx_handle, &amln);
            lai_var_move(&node->object, &object);
            LAI_TRY(lai_install_nsnode(node));

            struct lai_ctxitem *ctxitem = lai_exec_peek_ctxstack_back(state);
            if (ctxitem->invocation)
                lai_list_link(&ctxitem->invocation->per_method_list, &node->per_method_item);
            break;
        }
        case BITFIELD_OP:
        case BYTEFIELD_OP:
        case WORDFIELD_OP:
        case DWORDFIELD_OP:
        case QWORDFIELD_OP: {
            lai_variable_t offset = {0};
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &offset));
            LAI_ENSURE(operands[2].tag == LAI_UNRESOLVED_NAME);

            struct lai_amlname node_amln;
            lai_amlname_parse(&node_amln, operands[2].unres_aml);

            lai_nsnode_t *node = lai_create_nsnode_or_die();
            node->type = LAI_NAMESPACE_BUFFER_FIELD;
            lai_do_resolve_new_node(node, operands[2].unres_ctx_handle, &node_amln);

            LAI_CLEANUP_VAR lai_variable_t buf = LAI_VAR_INITIALIZER;
            lai_operand_load(state, &operands[0], &buf);
            node->bf_buffer = buf.buffer_ptr;
            lai_rc_ref(&node->bf_buffer->rc);

            switch (opcode) {
                case BITFIELD_OP:
                    node->bf_size = 1;
                    break;
                case BYTEFIELD_OP:
                    node->bf_size = 8;
                    break;
                case WORDFIELD_OP:
                    node->bf_size = 16;
                    break;
                case DWORDFIELD_OP:
                    node->bf_size = 32;
                    break;
                case QWORDFIELD_OP:
                    node->bf_size = 64;
                    break;
            }
            switch (opcode) {
                case BITFIELD_OP:
                    node->bf_offset = offset.integer;
                    break;
                case BYTEFIELD_OP:
                case WORDFIELD_OP:
                case DWORDFIELD_OP:
                case QWORDFIELD_OP:
                    node->bf_offset = offset.integer * 8;
                    break;
            }

            LAI_TRY(lai_install_nsnode(node));

            struct lai_ctxitem *ctxitem = lai_exec_peek_ctxstack_back(state);
            if (ctxitem->invocation)
                lai_list_link(&ctxitem->invocation->per_method_list, &node->per_method_item);
            break;
        }
        case (EXTOP_PREFIX << 8) | ARBFIELD_OP: {
            lai_variable_t offset = {0};
            lai_variable_t size = {0};
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &offset));
            LAI_TRY(lai_exec_get_integer(state, &operands[2], &size));

            LAI_ENSURE(operands[3].tag == LAI_UNRESOLVED_NAME);

            struct lai_amlname node_amln;
            lai_amlname_parse(&node_amln, operands[3].unres_aml);

            lai_nsnode_t *node = lai_create_nsnode_or_die();
            node->type = LAI_NAMESPACE_BUFFER_FIELD;
            lai_do_resolve_new_node(node, operands[3].unres_ctx_handle, &node_amln);

            LAI_CLEANUP_VAR lai_variable_t buf = LAI_VAR_INITIALIZER;
            lai_operand_load(state, &operands[0], &buf);
            node->bf_buffer = buf.buffer_ptr;
            lai_rc_ref(&node->bf_buffer->rc);

            node->bf_size = size.integer;
            node->bf_offset = offset.integer;

            LAI_TRY(lai_install_nsnode(node));

            struct lai_ctxitem *ctxitem = lai_exec_peek_ctxstack_back(state);
            if (ctxitem->invocation)
                lai_list_link(&ctxitem->invocation->per_method_list, &node->per_method_item);
            break;
        }
        case (EXTOP_PREFIX << 8) | OPREGION: {
            lai_variable_t base = {0};
            lai_variable_t size = {0};
            LAI_TRY(lai_exec_get_integer(state, &operands[2], &base));
            LAI_TRY(lai_exec_get_integer(state, &operands[3], &size));

            LAI_ENSURE(operands[0].tag == LAI_UNRESOLVED_NAME);
            LAI_ENSURE(operands[1].tag == LAI_OPERAND_OBJECT
                       && operands[1].object.type == LAI_INTEGER);

            struct lai_amlname amln;
            lai_amlname_parse(&amln, operands[0].unres_aml);

            lai_nsnode_t *node = lai_create_nsnode_or_die();
            lai_do_resolve_new_node(node, ctx_handle, &amln);
            node->type = LAI_NAMESPACE_OPREGION;
            node->op_address_space = operands[1].object.integer;
            node->op_base = base.integer;
            node->op_length = size.integer;

            LAI_TRY(lai_install_nsnode(node));

            struct lai_ctxitem *ctxitem = lai_exec_peek_ctxstack_back(state);
            if (ctxitem->invocation)
                lai_list_link(&ctxitem->invocation->per_method_list, &node->per_method_item);
            break;
        }
        default:
            lai_panic("undefined opcode in lai_exec_reduce_node: %02X", opcode);
    }

    return LAI_ERROR_NONE;
}

static lai_api_error_t lai_exec_reduce_op(int opcode, lai_state_t *state,
                                          struct lai_operand *operands,
                                          lai_variable_t *reduction_res) {
    if (lai_current_instance()->trace & LAI_TRACE_OP)
        lai_debug("lai_exec_reduce_op: opcode 0x%02X", opcode);
    lai_variable_t result = {0};
    switch (opcode) {
        case STORE_OP: {
            lai_variable_t objectref = {0};
            lai_variable_t out = {0};
            lai_exec_get_objectref(state, &operands[0], &objectref);

            lai_obj_clone(&result, &objectref);

            // Store a copy to the target operand.
            // TODO: Verify that we HAVE to make a copy.
            lai_obj_clone(&out, &result);
            lai_operand_mutate(state, &operands[1], &result);

            lai_var_finalize(&objectref);
            lai_var_finalize(&out);
            break;
        }
        case COPYOBJECT_OP: {
            lai_variable_t objectref = {0};
            lai_variable_t out = {0};
            lai_exec_get_objectref(state, &operands[0], &objectref);

            lai_obj_clone(&result, &objectref);

            // Store a copy to the target operand.
            lai_obj_clone(&out, &result);
            lai_operand_emplace(state, &operands[1], &result);

            lai_var_finalize(&objectref);
            lai_var_finalize(&out);
            break;
        }
        case NOT_OP: {
            lai_variable_t operand = {0};
            LAI_TRY(lai_exec_get_integer(state, operands, &operand));

            result.type = LAI_INTEGER;
            result.integer = ~operand.integer;
            lai_operand_mutate(state, &operands[1], &result);
            break;
        }
        case FINDSETLEFTBIT_OP: {
            LAI_CLEANUP_VAR lai_variable_t operand = LAI_VAR_INITIALIZER;
            LAI_TRY(lai_exec_get_integer(state, operands, &operand));

            result.type = LAI_INTEGER;
            int msb = 0;
            while (operand.integer != 0) {
                operand.integer >>= 1;
                msb++;
            }
            result.type = LAI_INTEGER;
            result.integer = msb;
            lai_operand_mutate(state, &operands[1], &result);
            break;
        }
        case FINDSETRIGHTBIT_OP: {
            LAI_CLEANUP_VAR lai_variable_t operand = LAI_VAR_INITIALIZER;
            LAI_TRY(lai_exec_get_integer(state, operands, &operand));

            result.type = LAI_INTEGER;
            int lsb = 0;
            while (operand.integer != 0) {
                ++lsb;
                operand.integer <<= 1;
            }
            result.type = LAI_INTEGER;
            result.integer = lsb == 0 ? 0 : (65 - lsb);
            lai_operand_mutate(state, &operands[1], &result);
            break;
        }
        case CONCAT_OP: {
            LAI_CLEANUP_VAR lai_variable_t operand0 = LAI_VAR_INITIALIZER;
            lai_exec_get_objectref(state, &operands[0], &operand0);
            LAI_CLEANUP_VAR lai_variable_t operand1 = LAI_VAR_INITIALIZER;
            lai_exec_get_objectref(state, &operands[1], &operand1);

            LAI_CLEANUP_VAR lai_variable_t operand0_convert = LAI_VAR_INITIALIZER;
            LAI_CLEANUP_VAR lai_variable_t operand1_convert_temp = LAI_VAR_INITIALIZER;
            LAI_CLEANUP_VAR lai_variable_t operand1_convert = LAI_VAR_INITIALIZER;

            /*
             * Convert object types other than integers, strings and buffers to a string.
             *
             * According to the ACPI specification buffer fields should be turned into
             * strings, but in ACPICA they are treated as either integers on strings
             * based on their size.
             */
            if (operand0.type != LAI_INTEGER && operand0.type != LAI_BUFFER
                && operand0.type != LAI_STRING) {
                if (operand0.type == LAI_HANDLE) {
                    lai_api_error_t error
                        = lai_obj_to_type_string(&operand0_convert, operand0.handle);
                    if (error != LAI_ERROR_NONE) {
                        lai_warn("Failed lai_obj_to_type_string: %s",
                                 lai_api_error_to_string(error));
                        return error;
                    }
                } else if (operand0.type == LAI_TYPE_NONE) {
                    lai_api_error_t error
                        = lai_create_c_string(&operand0_convert, "[Uninitialized Object]");
                    if (error != LAI_ERROR_NONE) {
                        lai_warn("failed to allocate memory for AML string");
                        return error;
                    }
                } else if (operand0.type == LAI_PACKAGE) {
                    lai_api_error_t error
                        = lai_create_c_string(&operand0_convert, "[Package Object]");
                    if (error != LAI_ERROR_NONE) {
                        lai_warn("failed to allocate memory for AML string");
                        return error;
                    }
                }
            } else {
                lai_obj_clone(&operand0_convert, &operand0);
            }

            if (operand1.type != LAI_INTEGER && operand1.type != LAI_BUFFER
                && operand1.type != LAI_STRING) {
                if (operand1.type == LAI_HANDLE) {
                    lai_api_error_t error = lai_create_string(&operand1_convert_temp, 0);
                    if (error != LAI_ERROR_NONE) {
                        lai_warn("failed to allocate memory for AML string");
                        return error;
                    }
                    error = lai_obj_to_type_string(&operand1_convert_temp, operand1.handle);
                    if (error != LAI_ERROR_NONE) {
                        lai_warn("Failed lai_obj_to_type_string: %s",
                                 lai_api_error_to_string(error));
                        return error;
                    }
                } else if (operand1.type == LAI_TYPE_NONE) {
                    lai_api_error_t error = lai_create_string(&operand1_convert_temp, 22);
                    if (error != LAI_ERROR_NONE) {
                        lai_warn("failed to allocate memory for AML string");
                        return error;
                    }
                    char *str = lai_exec_string_access(&operand1_convert_temp);
                    lai_strcpy(str, "[Uninitialized Object]");
                } else if (operand1.type == LAI_PACKAGE) {
                    lai_api_error_t error = lai_create_string(&operand1_convert_temp, 16);
                    if (error != LAI_ERROR_NONE) {
                        lai_warn("failed to allocate memory for AML string");
                        return error;
                    }
                    char *str = lai_exec_string_access(&operand1_convert_temp);
                    lai_strcpy(str, "[Package Object]");
                }
            } else {
                lai_obj_clone(&operand1_convert_temp, &operand1);
            }

            switch (operand0_convert.type) {
                case LAI_INTEGER: {
                    operand1_convert.type = LAI_INTEGER;
                    lai_api_error_t error
                        = lai_mutate_integer(&operand1_convert, &operand1_convert_temp);
                    if (error != LAI_ERROR_NONE) {
                        lai_warn("Failed lai_mutate_integer: %s", lai_api_error_to_string(error));
                        return error;
                    }
                    error = lai_create_buffer(&result, sizeof(uint64_t) * 2);
                    if (error != LAI_ERROR_NONE) {
                        lai_warn("failed to allocate memory for AML buffer");
                        return error;
                    }
                    uint64_t *buffer = lai_exec_buffer_access(&result);
                    buffer[0] = operand0_convert.integer;
                    buffer[1] = operand1_convert.integer;
                    result.type = LAI_BUFFER;
                    break;
                }
                case LAI_BUFFER: {
                    if (operand1_convert_temp.type == LAI_STRING) {
                        size_t strl = lai_exec_string_length(&operand1_convert_temp);
                        lai_api_error_t error = lai_create_buffer(&operand1_convert, strl + 1);
                        if (error != LAI_ERROR_NONE) {
                            lai_warn("failed to allocate memory for AML buffer");
                            return error;
                        }
                        error = lai_mutate_buffer(&operand1_convert, &operand1_convert_temp);
                        if (error != LAI_ERROR_NONE) {
                            lai_warn("Failed lai_mutate_buffer: %s",
                                     lai_api_error_to_string(error));
                            return error;
                        }
                    } else if (operand1_convert_temp.type == LAI_INTEGER) {
                        lai_api_error_t error
                            = lai_create_buffer(&operand1_convert, sizeof(uint64_t));
                        if (error != LAI_ERROR_NONE) {
                            lai_warn("failed to allocate memory for AML buffer");
                            return error;
                        }
                        error = lai_mutate_buffer(&operand1_convert, &operand1_convert_temp);
                        if (error != LAI_ERROR_NONE) {
                            lai_warn("Failed lai_mutate_buffer: %s",
                                     lai_api_error_to_string(error));
                            return error;
                        }
                    } else if (operand1_convert_temp.type == LAI_BUFFER) {
                        lai_obj_clone(&operand1_convert, &operand1_convert_temp);
                    }
                    size_t b0size = lai_exec_buffer_size(&operand0_convert);
                    size_t b1size = lai_exec_buffer_size(&operand1_convert);
                    lai_api_error_t error = lai_create_buffer(&result, b0size + b1size);
                    if (error != LAI_ERROR_NONE) {
                        lai_warn("Failed to allocate memory for AML buffer");
                        return error;
                    }
                    char *buffer0 = lai_exec_buffer_access(&operand0_convert);
                    char *buffer1 = lai_exec_buffer_access(&operand1_convert);
                    char *result_buffer = lai_exec_buffer_access(&result);
                    memcpy(result_buffer, buffer0, b0size);
                    memcpy(result_buffer + b0size, buffer1, b0size);
                    result.type = LAI_BUFFER;
                    break;
                }
                case LAI_STRING: {
                    switch (operand1_convert_temp.type) {
                        case LAI_STRING: {
                            lai_obj_clone(&operand1_convert, &operand1_convert_temp);
                            break;
                        }
                        case LAI_INTEGER: {
                            lai_api_error_t error = lai_create_string(&operand1_convert, 0);
                            if (error != LAI_ERROR_NONE) {
                                lai_warn("failed to allocate memory for AML string");
                                return error;
                            }
                            error = lai_mutate_string(&operand1_convert, &operand1_convert_temp);
                            if (error != LAI_ERROR_NONE) {
                                lai_panic("Failed lai_mutate_string: %s",
                                          lai_api_error_to_string(error));
                                return error;
                            }
                            break;
                        }
                        case LAI_BUFFER: {
                            // size_t length = lai_exec_buffer_size(&operand1_convert_temp);
                            lai_api_error_t error = lai_create_string(&operand1_convert, 0);
                            if (error != LAI_ERROR_NONE) {
                                lai_warn("failed to allocate memory for AML string");
                                return error;
                            }
                            error = lai_mutate_string(&operand1_convert, &operand1_convert_temp);
                            if (error != LAI_ERROR_NONE) {
                                lai_warn("Failed lai_mutate_string: %s",
                                         lai_api_error_to_string(error));
                                return error;
                            }
                            break;
                        }
                    }
                    size_t s0len = lai_exec_string_length(&operand0_convert);
                    size_t s1len = lai_exec_string_length(&operand1_convert);
                    lai_api_error_t error = lai_create_string(&result, s0len + s1len + 1);
                    if (error != LAI_ERROR_NONE) {
                        lai_warn("failed to allocate memory for AML string");
                        return error;
                    }
                    char *string0 = lai_exec_string_access(&operand0_convert);
                    char *string1 = lai_exec_string_access(&operand1_convert);
                    char *result_string = lai_exec_string_access(&result);
                    memcpy(result_string, string0, s0len);
                    memcpy(result_string + s0len, string1, s1len);
                    result_string[s0len + s1len + 1] = '\0';
                    result.type = LAI_STRING;
                }
            }
            lai_operand_emplace(state, &operands[2], &result);
            break;
        }
        case ADD_OP: {
            lai_variable_t lhs = {0};
            lai_variable_t rhs = {0};
            LAI_TRY(lai_exec_get_integer(state, &operands[0], &lhs));
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &rhs));

            result.type = LAI_INTEGER;
            result.integer = lhs.integer + rhs.integer;
            lai_operand_mutate(state, &operands[2], &result);
            break;
        }
        case SUBTRACT_OP: {
            lai_variable_t lhs = {0};
            lai_variable_t rhs = {0};
            LAI_TRY(lai_exec_get_integer(state, &operands[0], &lhs));
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &rhs));

            result.type = LAI_INTEGER;
            result.integer = lhs.integer - rhs.integer;
            lai_operand_mutate(state, &operands[2], &result);
            break;
        }
        case MOD_OP: {
            lai_variable_t lhs = {0};
            lai_variable_t rhs = {0};
            LAI_TRY(lai_exec_get_integer(state, &operands[0], &lhs));
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &rhs));

            result.type = LAI_INTEGER;
            result.integer = lhs.integer % rhs.integer;
            lai_operand_mutate(state, &operands[2], &result);
            break;
        }
        case MULTIPLY_OP: {
            lai_variable_t lhs = {0};
            lai_variable_t rhs = {0};
            LAI_TRY(lai_exec_get_integer(state, &operands[0], &lhs));
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &rhs));

            result.type = LAI_INTEGER;
            result.integer = lhs.integer * rhs.integer;
            lai_operand_mutate(state, &operands[2], &result);
            break;
        }
        case AND_OP: {
            lai_variable_t lhs = {0};
            lai_variable_t rhs = {0};
            LAI_TRY(lai_exec_get_integer(state, &operands[0], &lhs));
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &rhs));

            result.type = LAI_INTEGER;
            result.integer = lhs.integer & rhs.integer;
            lai_operand_mutate(state, &operands[2], &result);
            break;
        }
        case OR_OP: {
            lai_variable_t lhs = {0};
            lai_variable_t rhs = {0};
            LAI_TRY(lai_exec_get_integer(state, &operands[0], &lhs));
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &rhs));

            result.type = LAI_INTEGER;
            result.integer = lhs.integer | rhs.integer;
            lai_operand_mutate(state, &operands[2], &result);
            break;
        }
        case XOR_OP: {
            lai_variable_t lhs = {0};
            lai_variable_t rhs = {0};
            LAI_TRY(lai_exec_get_integer(state, &operands[0], &lhs));
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &rhs));

            result.type = LAI_INTEGER;
            result.integer = lhs.integer ^ rhs.integer;
            lai_operand_mutate(state, &operands[2], &result);
            break;
        }
        case NAND_OP: {
            lai_variable_t lhs = {0};
            lai_variable_t rhs = {0};
            LAI_TRY(lai_exec_get_integer(state, &operands[0], &lhs));
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &rhs));

            result.type = LAI_INTEGER;
            result.integer = ~(lhs.integer & rhs.integer);
            lai_operand_mutate(state, &operands[2], &result);
            break;
        }
        case NOR_OP: {
            lai_variable_t lhs = {0};
            lai_variable_t rhs = {0};
            LAI_TRY(lai_exec_get_integer(state, &operands[0], &lhs));
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &rhs));

            result.type = LAI_INTEGER;
            result.integer = ~(lhs.integer | rhs.integer);
            lai_operand_mutate(state, &operands[2], &result);
            break;
        }
        case SHL_OP: {
            lai_variable_t lhs = {0};
            lai_variable_t rhs = {0};
            LAI_TRY(lai_exec_get_integer(state, &operands[0], &lhs));
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &rhs));

            result.type = LAI_INTEGER;
            result.integer = lhs.integer << rhs.integer;
            lai_operand_mutate(state, &operands[2], &result);
            break;
        }
        case SHR_OP: {
            lai_variable_t lhs = {0};
            lai_variable_t rhs = {0};
            LAI_TRY(lai_exec_get_integer(state, &operands[0], &lhs));
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &rhs));

            result.type = LAI_INTEGER;
            result.integer = lhs.integer >> rhs.integer;
            lai_operand_mutate(state, &operands[2], &result);
            break;
        }
        case DIVIDE_OP: {
            lai_variable_t lhs = {0};
            lai_variable_t rhs = {0};
            LAI_TRY(lai_exec_get_integer(state, &operands[0], &lhs));
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &rhs));

            lai_variable_t mod = {0};
            lai_variable_t div = {0};
            mod.type = LAI_INTEGER;
            div.type = LAI_INTEGER;
            mod.integer = lhs.integer % rhs.integer;
            div.integer = lhs.integer / rhs.integer;
            lai_operand_mutate(state, &operands[2], &mod);
            lai_operand_mutate(state, &operands[3], &div);
            break;
        }
        case INCREMENT_OP: {
            lai_operand_load(state, operands, &result);
            LAI_ENSURE(result.type == LAI_INTEGER);
            result.integer++;
            lai_operand_mutate(state, operands, &result);
            break;
        }
        case DECREMENT_OP: {
            lai_operand_load(state, operands, &result);
            LAI_ENSURE(result.type == LAI_INTEGER);
            result.integer--;
            lai_operand_mutate(state, operands, &result);
            break;
        }
        case LNOT_OP: {
            lai_variable_t operand = {0};
            LAI_TRY(lai_exec_get_integer(state, operands, &operand));

            result.type = LAI_INTEGER;
            result.integer = !operand.integer;
            break;
        }
        case LAND_OP: {
            lai_variable_t lhs = {0};
            lai_variable_t rhs = {0};
            LAI_TRY(lai_exec_get_integer(state, &operands[0], &lhs));
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &rhs));

            result.type = LAI_INTEGER;
            result.integer = lhs.integer && rhs.integer;
            break;
        }
        case LOR_OP: {
            lai_variable_t lhs = {0};
            lai_variable_t rhs = {0};
            LAI_TRY(lai_exec_get_integer(state, &operands[0], &lhs));
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &rhs));

            result.type = LAI_INTEGER;
            result.integer = lhs.integer || rhs.integer;
            break;
        }
        case LEQUAL_OP: {
            LAI_CLEANUP_VAR lai_variable_t lhs = LAI_VAR_INITIALIZER;
            lai_exec_get_objectref(state, &operands[0], &lhs);

            LAI_CLEANUP_VAR lai_variable_t rhs = LAI_VAR_INITIALIZER;
            lai_exec_get_objectref(state, &operands[1], &rhs);

            int res = 0;
            lai_api_error_t err = lai_obj_exec_match_op(MATCH_MEQ, &lhs, &rhs, &res);
            if (err != LAI_ERROR_NONE)
                return LAI_ERROR_ILLEGAL_ARGUMENTS;

            result.type = LAI_INTEGER;
            result.integer = res ? ~((uint64_t)0) : 0;

            break;
        }
        case LLESS_OP: {
            LAI_CLEANUP_VAR lai_variable_t lhs = LAI_VAR_INITIALIZER;
            lai_exec_get_objectref(state, &operands[0], &lhs);

            LAI_CLEANUP_VAR lai_variable_t rhs = LAI_VAR_INITIALIZER;
            lai_exec_get_objectref(state, &operands[1], &rhs);

            int res = 0;
            lai_api_error_t err = lai_obj_exec_match_op(MATCH_MLT, &lhs, &rhs, &res);
            if (err != LAI_ERROR_NONE)
                return LAI_ERROR_ILLEGAL_ARGUMENTS;

            result.type = LAI_INTEGER;
            result.integer = res ? ~((uint64_t)0) : 0;

            break;
        }
        case LGREATER_OP: {
            LAI_CLEANUP_VAR lai_variable_t lhs = LAI_VAR_INITIALIZER;
            lai_exec_get_objectref(state, &operands[0], &lhs);

            LAI_CLEANUP_VAR lai_variable_t rhs = LAI_VAR_INITIALIZER;
            lai_exec_get_objectref(state, &operands[1], &rhs);

            int res = 0;
            lai_api_error_t err = lai_obj_exec_match_op(MATCH_MGT, &lhs, &rhs, &res);
            if (err != LAI_ERROR_NONE)
                return LAI_ERROR_ILLEGAL_ARGUMENTS;

            result.type = LAI_INTEGER;
            result.integer = res ? ~((uint64_t)0) : 0;

            break;
        }
        case INDEX_OP: {
            lai_variable_t object = {0};
            lai_variable_t index = {0};
            lai_exec_get_objectref(state, &operands[0], &object);
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &index));
            size_t n = index.integer;

            switch (object.type) {
                case LAI_STRING:
                    if (n >= lai_exec_string_length(&object))
                        lai_panic("string Index() out of bounds");
                    result.type = LAI_STRING_INDEX;
                    result.string_ptr = object.string_ptr;
                    lai_rc_ref(&object.string_ptr->rc);
                    result.integer = n;
                    break;
                case LAI_BUFFER:
                    if (n >= lai_exec_buffer_size(&object))
                        lai_panic("buffer Index() out of bounds");
                    result.type = LAI_BUFFER_INDEX;
                    result.buffer_ptr = object.buffer_ptr;
                    lai_rc_ref(&object.buffer_ptr->rc);
                    result.integer = n;
                    break;
                case LAI_PACKAGE:
                    if (n >= lai_exec_pkg_size(&object))
                        lai_panic("package Index() out of bounds");
                    result.type = LAI_PACKAGE_INDEX;
                    result.pkg_ptr = object.pkg_ptr;
                    result.integer = n;
                    lai_rc_ref(&object.pkg_ptr->rc);
                    break;
                default:
                    lai_panic("Index() is only defined for buffers, strings and packages"
                              " but object of type %d was given",
                              object.type);
            }
            lai_var_finalize(&object);

            // TODO: Verify that we do NOT have to make a copy.
            lai_operand_mutate(state, &operands[2], &result);
            break;
        }
        case MATCH_OP: {
            LAI_CLEANUP_VAR lai_variable_t package = LAI_VAR_INITIALIZER;
            lai_exec_get_objectref(state, &operands[0], &package);
            if (package.type != LAI_PACKAGE)
                return LAI_ERROR_UNEXPECTED_RESULT;

            LAI_CLEANUP_VAR lai_variable_t op1_var = LAI_VAR_INITIALIZER;
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &op1_var));
            size_t op1 = op1_var.integer;

            LAI_CLEANUP_VAR lai_variable_t object1 = LAI_VAR_INITIALIZER;
            lai_exec_get_objectref(state, &operands[2], &object1);

            LAI_CLEANUP_VAR lai_variable_t op2_var = LAI_VAR_INITIALIZER;
            LAI_TRY(lai_exec_get_integer(state, &operands[3], &op2_var));
            size_t op2 = op2_var.integer;

            LAI_CLEANUP_VAR lai_variable_t object2 = LAI_VAR_INITIALIZER;
            lai_exec_get_objectref(state, &operands[4], &object2);

            LAI_CLEANUP_VAR lai_variable_t start_index_var = LAI_VAR_INITIALIZER;
            LAI_TRY(lai_exec_get_integer(state, &operands[5], &start_index_var));
            size_t start_index = start_index_var.integer;

            result.type = LAI_INTEGER;
            result.integer = ~((uint64_t)0); // OnesOp

            size_t package_size = lai_exec_pkg_size(&package);
            for (size_t i = start_index; i < package_size; i++) {
                LAI_CLEANUP_VAR lai_variable_t object = LAI_VAR_INITIALIZER;
                lai_exec_pkg_load(&object, &package, i);

                int a = 0;
                lai_api_error_t res = lai_obj_exec_match_op(op1, &object, &object1, &a);
                if (res != LAI_ERROR_NONE)
                    return LAI_ERROR_ILLEGAL_ARGUMENTS;

                int b = 0;
                res = lai_obj_exec_match_op(op2, &object, &object2, &b);
                if (res != LAI_ERROR_NONE)
                    return LAI_ERROR_ILLEGAL_ARGUMENTS;

                if (a && b) {
                    result.integer = i;
                    break;
                }
            }

            break;
        }
        case CONCATRES_OP: {
            LAI_CLEANUP_VAR lai_variable_t buf1_var = LAI_VAR_INITIALIZER;
            lai_exec_get_objectref(state, &operands[0], &buf1_var);

            LAI_CLEANUP_VAR lai_variable_t buf2_var = LAI_VAR_INITIALIZER;
            lai_exec_get_objectref(state, &operands[1], &buf2_var);

            size_t buf1_size = lai_exec_buffer_size(&buf1_var);
            const char *buf1 = lai_exec_buffer_access(&buf1_var);

            size_t buf2_size = lai_exec_buffer_size(&buf2_var);
            const char *buf2 = lai_exec_buffer_access(&buf2_var);

            // Forbidden as per spec
            if (buf1_size == 1 || buf2_size == 1)
                return LAI_ERROR_UNEXPECTED_RESULT;

            if (buf1_size == 0)
                buf1_size
                    = 2; // Make it 2 so memcpy will actually copy 0 zero bytes since it is empty

            if (buf2_size == 0)
                buf2_size = 2;

            size_t result_size = (buf1_size - 2) + (buf2_size - 2)
                                 + 2; // (buf1_size - end tag) + (buf2_size - end tag) + end tag
            lai_create_buffer(&result, result_size);
            char *result_buffer = lai_exec_buffer_access(&result);

            memcpy(result_buffer, buf1, buf1_size - 2);
            memcpy(result_buffer + (buf1_size - 2), buf2, buf2_size - 2);
            result_buffer[(buf1_size - 2) + (buf2_size - 2)] = 0x79; // Small End Tag

            // Calculate checksum to put into the End Tag
            uint8_t check = 0;
            for (size_t i = 0; i < (result_size - 1); i++)
                check += result_buffer[i];

            result_buffer[(buf1_size - 2) + (buf2_size - 2) + 1] = 256 - check;

            lai_operand_emplace(state, &operands[2], &result);
            break;
        }
        case DEREF_OP: {
            lai_variable_t ref = {0};
            lai_exec_get_objectref(state, &operands[0], &ref);

            switch (ref.type) {
                // TODO: DeRefOf() can take strings!
                //       It resolves them to objects and returns the contents.
                case LAI_ARG_REF:
                case LAI_LOCAL_REF:
                case LAI_NODE_REF: {
                    LAI_CLEANUP_VAR lai_variable_t temp = LAI_VAR_INITIALIZER;
                    lai_exec_ref_load(&temp, &ref);
                    lai_obj_clone(&result, &temp);
                    break;
                }
                case LAI_STRING_INDEX: {
                    char *window = ref.string_ptr->content;
                    result.type = LAI_INTEGER;
                    result.integer = window[ref.integer];
                    break;
                }
                case LAI_BUFFER_INDEX: {
                    uint8_t *window = ref.buffer_ptr->content;
                    result.type = LAI_INTEGER;
                    result.integer = window[ref.integer];
                    break;
                }
                case LAI_PACKAGE_INDEX:
                    // TODO: We need to panic if we load an uninitialized entry.
                    lai_exec_pkg_var_load(&result, ref.pkg_ptr, ref.integer);
                    break;
                default:
                    lai_panic("Unexpected object type %d for DeRefOf()", ref.type);
            }

            break;
        }
        case SIZEOF_OP: {
            lai_variable_t object = {0};
            lai_exec_get_objectref(state, &operands[0], &object);

            switch (object.type) {
                case LAI_STRING:
                    result.type = LAI_INTEGER;
                    result.integer = lai_exec_string_length(&object);
                    break;
                case LAI_BUFFER:
                    result.type = LAI_INTEGER;
                    result.integer = lai_exec_buffer_size(&object);
                    break;
                case LAI_PACKAGE:
                    result.type = LAI_INTEGER;
                    result.integer = lai_exec_pkg_size(&object);
                    break;
                default:
                    lai_panic("SizeOf() is only defined for buffers, strings and packages");
            }

            break;
        }
        case REFOF_OP: {
            struct lai_operand *operand = &operands[0];

            // TODO: The resolution code should be shared with CONDREF_OP.
            lai_variable_t ref = {0};
            switch (operand->tag) {
                case LAI_ARG_NAME: {
                    struct lai_ctxitem *ctxitem = lai_exec_peek_ctxstack_back(state);
                    LAI_ENSURE(ctxitem->invocation);
                    ref.type = LAI_ARG_REF;
                    ref.iref_invocation = ctxitem->invocation;
                    ref.iref_index = operand->index;
                    break;
                }
                case LAI_LOCAL_NAME: {
                    struct lai_ctxitem *ctxitem = lai_exec_peek_ctxstack_back(state);
                    LAI_ENSURE(ctxitem->invocation);
                    ref.type = LAI_LOCAL_REF;
                    ref.iref_invocation = ctxitem->invocation;
                    ref.iref_index = operand->index;
                    break;
                }
                case LAI_RESOLVED_NAME:
                    ref.type = LAI_NODE_REF;
                    ref.handle = operand->handle;
                    break;
                default:
                    lai_panic("Unexpected operand tag %d for RefOf()", operand->tag);
            }

            lai_var_move(&result, &ref);
            break;
        }
        case TOBUFFER_OP: {
            LAI_CLEANUP_VAR lai_variable_t operand = LAI_VAR_INITIALIZER;
            lai_exec_get_objectref(state, &operands[0], &operand);

            lai_api_error_t error = lai_obj_to_buffer(&result, &operand);
            if (error != LAI_ERROR_NONE)
                lai_panic("Failed ToBuffer: %s", lai_api_error_to_string(error));

            lai_operand_emplace(state, &operands[1], &result);
            break;
        }
        case TODECIMALSTRING_OP: {
            LAI_CLEANUP_VAR lai_variable_t operand = LAI_VAR_INITIALIZER;
            lai_exec_get_objectref(state, &operands[0], &operand);

            lai_api_error_t error = lai_obj_to_decimal_string(&result, &operand);
            if (error != LAI_ERROR_NONE)
                lai_panic("Failed ToDecimalString: %s", lai_api_error_to_string(error));

            lai_operand_emplace(state, &operands[1], &result);
            break;
        }
        case TOHEXSTRING_OP: {
            LAI_CLEANUP_VAR lai_variable_t operand = LAI_VAR_INITIALIZER;
            lai_exec_get_objectref(state, &operands[0], &operand);

            lai_api_error_t error = lai_obj_to_hex_string(&result, &operand);
            if (error != LAI_ERROR_NONE)
                lai_panic("Failed ToHexString: %s", lai_api_error_to_string(error));

            lai_operand_emplace(state, &operands[1], &result);
            break;
        }
        case TOINTEGER_OP: {
            LAI_CLEANUP_VAR lai_variable_t operand = LAI_VAR_INITIALIZER;
            lai_exec_get_objectref(state, &operands[0], &operand);

            lai_api_error_t error = lai_obj_to_integer(&result, &operand);
            if (error != LAI_ERROR_NONE)
                lai_panic("Failed ToInteger: %s", lai_api_error_to_string(error));

            lai_operand_emplace(state, &operands[1], &result);
            break;
        }
        case TOSTRING_OP: {
            LAI_CLEANUP_VAR lai_variable_t operand = LAI_VAR_INITIALIZER;
            lai_exec_get_objectref(state, &operands[0], &operand);
            LAI_CLEANUP_VAR lai_variable_t size_var = LAI_VAR_INITIALIZER;
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &size_var));

            lai_api_error_t error = lai_obj_to_string(&result, &operand, size_var.integer);
            if (error != LAI_ERROR_NONE)
                lai_panic("Failed ToString: %s", lai_api_error_to_string(error));

            lai_operand_emplace(state, &operands[2], &result);
            break;
        }
        case MID_OP: {
            LAI_CLEANUP_VAR lai_variable_t object = LAI_VAR_INITIALIZER;
            lai_exec_get_objectref(state, &operands[0], &object);
            LAI_CLEANUP_VAR lai_variable_t index = LAI_VAR_INITIALIZER;
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &index));
            LAI_CLEANUP_VAR lai_variable_t length = LAI_VAR_INITIALIZER;
            LAI_TRY(lai_exec_get_integer(state, &operands[2], &length));

            size_t strl = 0;
            size_t n = index.integer;
            size_t sz = length.integer;
            if (object.type == LAI_STRING) {
                strl = lai_exec_string_length(&object);
            } else if (object.type == LAI_BUFFER) {
                strl = lai_exec_buffer_size(&object);
            }

            if (n >= strl) {
                sz = 0;
                /*
                 * NOTE: The spec states that "if Index + Length is greater than or equal...",
                 * however ACPICA only checks if is greater than so we do the same.
                 */
            } else if ((n + sz) > strl) {
                sz = strl - n;
            }

            switch (object.type) {
                case LAI_STRING: {
                    lai_api_error_t error = lai_create_string(&result, sz + 1);
                    if (error != LAI_ERROR_NONE) {
                        lai_warn("failed to allocate memory for AML buffer");
                        return error;
                    }
                    char *buffer0 = lai_exec_string_access(&object);
                    char *result_string = lai_exec_string_access(&result);
                    memcpy(result_string, buffer0 + n, sz);
                    result.type = LAI_STRING;
                    break;
                }
                case LAI_BUFFER: {
                    lai_api_error_t error = lai_create_buffer(&result, sz);
                    if (error != LAI_ERROR_NONE) {
                        lai_warn("failed to allocate memory for AML buffer");
                        return error;
                    }
                    char *buffer0 = lai_exec_buffer_access(&object);
                    char *result_buffer = lai_exec_buffer_access(&result);
                    memcpy(result_buffer, buffer0 + n, sz);
                    result.type = LAI_BUFFER;
                    break;
                }
            }

            lai_operand_mutate(state, &operands[3], &result);
            break;
        }
        case NOTIFY_OP: {
            LAI_CLEANUP_VAR lai_variable_t code = LAI_VAR_INITIALIZER;
            LAI_ENSURE(operands[0].tag == LAI_RESOLVED_NAME);
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &code));

            lai_nsnode_t *node = operands[0].handle;
            LAI_ENSURE(node->type == LAI_NAMESPACE_DEVICE || node->type == LAI_NAMESPACE_PROCESSOR
                       || node->type == LAI_NAMESPACE_THERMALZONE);

            if (laihost_handle_global_notify)
                laihost_handle_global_notify(node, code.integer);

            if (node->notify_override) {
                lai_api_error_t error;
                error = node->notify_override(node, code.integer, node->notify_userptr);
                // TODO: for now, there no errors defined.
                //       Add a way for the host to signal Notify() failure.
                LAI_ENSURE(!error);
            } else {
                LAI_CLEANUP_FREE_STRING char *path = lai_stringify_node_path(node);
                lai_warn("Unhandled Notify(%s, 0x%lx)", path, code.integer);
            }
            break;
        }

        case (EXTOP_PREFIX << 8) | CONDREF_OP: {
            struct lai_operand *operand = &operands[0];
            struct lai_operand *target = &operands[1];

            // TODO: The resolution code should be shared with REFOF_OP.
            lai_variable_t ref = {0};
            switch (operand->tag) {
                case LAI_RESOLVED_NAME:
                    if (operand->handle) {
                        ref.type = LAI_HANDLE;
                        ref.handle = operand->handle;
                    }
                    break;
                default:
                    lai_panic("Unexpected operand tag %d for CondRefOf()", operand->tag);
            }

            if (ref.type) {
                result.type = LAI_INTEGER;
                result.integer = 1;
                lai_operand_mutate(state, target, &ref);
            } else {
                result.type = LAI_INTEGER;
                result.integer = 0;
            }

            break;
        }
        case (EXTOP_PREFIX << 8) | STALL_OP: {
            if (!laihost_timer)
                lai_panic("host does not provide timer functions required by Stall()");

            LAI_CLEANUP_VAR lai_variable_t time = {0};
            LAI_TRY(lai_exec_get_integer(state, &operands[0], &time));

            if (!time.integer)
                time.integer = 1;

            if (time.integer > 100) {
                lai_warn("buggy BIOS tried to stall for more than 100ms, using sleep instead");
                laihost_sleep(time.integer * 1000);
            } else {
                // use the timer to stall
                uint64_t start_time = laihost_timer();
                while (laihost_timer() - start_time <= time.integer * 10)
                    ;
            }
            break;
        }
        case (EXTOP_PREFIX << 8) | SLEEP_OP: {
            if (!laihost_sleep)
                lai_panic("host does not provide timer functions required by Sleep()");

            LAI_CLEANUP_VAR lai_variable_t time = {0};
            LAI_TRY(lai_exec_get_integer(state, &operands[0], &time));

            if (!time.integer)
                time.integer = 1;
            laihost_sleep(time.integer);
            break;
        }
        case (EXTOP_PREFIX << 8) | FATAL_OP: {
            LAI_CLEANUP_VAR lai_variable_t fatal_type = LAI_VAR_INITIALIZER;
            LAI_TRY(lai_exec_get_integer(state, &operands[0], &fatal_type));

            LAI_CLEANUP_VAR lai_variable_t fatal_data = LAI_VAR_INITIALIZER;
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &fatal_data));

            LAI_CLEANUP_VAR lai_variable_t fatal_arg = LAI_VAR_INITIALIZER;
            LAI_TRY(lai_exec_get_integer(state, &operands[2], &fatal_arg));

            if (!fatal_type.integer)
                fatal_type.integer = 0;

            if (!fatal_data.integer)
                fatal_data.integer = 0;

            if (!fatal_arg.integer)
                fatal_arg.integer = 0;

            lai_panic("FatalOp in AML, Type: %02lx, Data: %08lX, Arg: %lx\n", fatal_type.integer,
                      fatal_data.integer, fatal_arg.integer);
            break;
        }

        case (EXTOP_PREFIX << 8) | ACQUIRE_OP: {
            LAI_CLEANUP_VAR lai_variable_t timeout = LAI_VAR_INITIALIZER;
            LAI_ENSURE(operands[0].tag == LAI_RESOLVED_NAME);
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &timeout));

            lai_nsnode_t *node = operands[0].handle;
            LAI_ENSURE(node->type == LAI_NAMESPACE_MUTEX);

            if (lai_mutex_lock(&node->mut_sync, timeout.integer)) {
                result.type = LAI_INTEGER;
                result.integer = 1;
            } else {
                result.type = LAI_INTEGER;
                result.integer = 0;
            }
            break;
        }
        case (EXTOP_PREFIX << 8) | RELEASE_OP: {
            LAI_ENSURE(operands[0].tag == LAI_RESOLVED_NAME);

            lai_nsnode_t *node = operands[0].handle;
            LAI_ENSURE(node->type == LAI_NAMESPACE_MUTEX);

            lai_mutex_unlock(&node->mut_sync);
            break;
        }

        case (EXTOP_PREFIX << 8) | WAIT_OP: {
            LAI_CLEANUP_VAR lai_variable_t timeout = LAI_VAR_INITIALIZER;
            LAI_ENSURE(operands[0].tag == LAI_RESOLVED_NAME);
            LAI_TRY(lai_exec_get_integer(state, &operands[1], &timeout));

            lai_nsnode_t *node = operands[0].handle;
            LAI_ENSURE(node->type == LAI_NAMESPACE_EVENT);

            if (lai_event_wait(&node->evt_sync, timeout.integer)) {
                result.type = LAI_INTEGER;
                result.integer = 1;
            } else {
                result.type = LAI_INTEGER;
                result.integer = 0;
            }
            break;
        }
        case (EXTOP_PREFIX << 8) | SIGNAL_OP: {
            LAI_ENSURE(operands[0].tag == LAI_RESOLVED_NAME);

            lai_nsnode_t *node = operands[0].handle;
            LAI_ENSURE(node->type == LAI_NAMESPACE_EVENT);

            lai_event_signal(&node->evt_sync);
            break;
        }
        case (EXTOP_PREFIX << 8) | RESET_OP: {
            LAI_ENSURE(operands[0].tag == LAI_RESOLVED_NAME);

            lai_nsnode_t *node = operands[0].handle;
            LAI_ENSURE(node->type == LAI_NAMESPACE_EVENT);

            lai_event_reset(&node->evt_sync);
            break;
        }

        case (EXTOP_PREFIX << 8) | FROM_BCD_OP: {
            LAI_CLEANUP_VAR lai_variable_t operand = LAI_VAR_INITIALIZER;
            lai_exec_get_objectref(state, &operands[0], &operand);

            result.type = LAI_INTEGER;

            uint64_t power_of_ten = 1;
            uint64_t value = operand.integer;

            for (int i = 0; i < 16;
                 i++, power_of_ten *= 10) { // 16 is the amount of nibbles in 64bit
                uint8_t temp = (value >> (i * 4)) & 0xF;
                if (temp > 9) {
                    lai_warn("FromBCDOp Nibble is larger than 9 and thus an invalid BCD nibble");
                    // return LAI_ERROR_UNEXPECTED_RESULT;
                }

                result.integer += (temp * power_of_ten);
            }

            lai_operand_emplace(state, &operands[1], &result);
            break;
        }
        case (EXTOP_PREFIX << 8) | TO_BCD_OP: {
            LAI_CLEANUP_VAR lai_variable_t operand = LAI_VAR_INITIALIZER;
            lai_exec_get_objectref(state, &operands[0], &operand);

            result.type = LAI_INTEGER;
            result.integer = 0;

            // a uint64_t value can be expressed with 20 or less decimal digits
            uint64_t o = operand.integer;
            for (int i = 0; i < 20; i++) {
                result.integer = (result.integer << 4) | (o % 10);
                o /= 10;
                if (!o)
                    break;
            }

            lai_operand_emplace(state, &operands[1], &result);
            break;
        }
        case OBJECTTYPE_OP: {
            // NOTE: The spec states that predefined names (such as \_SB_ and others) are undefined
            // and return 0, however ACPICA doesn't do this so we don't either
            result.type = LAI_INTEGER;
            result.integer = 0;
            if (operands[0].tag == LAI_RESOLVED_NAME) {
                if (operands[0].handle->type == LAI_NAMESPACE_ALIAS)
                    result.integer = lai_objecttype_ns(operands[0].handle->al_target);
                else if (operands[0].handle->type == LAI_NAMESPACE_NAME
                         && operands[0].handle->object.type == LAI_NODE_REF)
                    result.integer = lai_objecttype_ns(operands[0].handle->object.handle);
                else
                    result.integer = lai_objecttype_ns(operands[0].handle);
            } else if (operands[0].tag == LAI_OPERAND_OBJECT) {
                lai_nsnode_t *node = operands[0].object.handle;
                if (node->type == LAI_NAMESPACE_ALIAS)
                    result.integer = lai_objecttype_ns(node->al_target);
                else if (node->type == LAI_NAMESPACE_NAME && node->object.type == LAI_NODE_REF)
                    result.integer = lai_objecttype_ns(node->object.handle);
                else
                    result.integer = lai_objecttype_ns(node);
            } else if (operands[0].tag == LAI_ARG_NAME || operands[0].tag == LAI_LOCAL_NAME) {
                LAI_CLEANUP_VAR lai_variable_t var = LAI_VAR_INITIALIZER;
                lai_operand_load(state, &operands[0], &var);

                result.integer = lai_objecttype_obj(&var);
            } else if (operands[0].tag == LAI_DEBUG_NAME) {
                result.integer = 16;
            }
            break;
        }

        default:
            lai_warn("undefined opcode in lai_exec_reduce_op: %02X", opcode);
            return LAI_ERROR_UNSUPPORTED;
    }

    lai_var_move(reduction_res, &result);
    return LAI_ERROR_NONE;
}

// lai_exec_run(): This is the main AML interpreter function.
static lai_api_error_t lai_exec_run(lai_state_t *state) {
    while (lai_exec_peek_stack_back(state)) {
        if (debug_stack)
            for (int i = 0;; i++) {
                lai_stackitem_t *trace_item = lai_exec_peek_stack(state, i);
                if (!trace_item)
                    break;
                switch (trace_item->kind) {
                    case LAI_OP_STACKITEM:
                        lai_debug("stack item %d is of type %d, opcode is 0x%x", i,
                                  trace_item->kind, trace_item->op_opcode);
                        break;
                    default:
                        lai_debug("stack item %d is of type %d", i, trace_item->kind);
                }
            }

        lai_api_error_t e;
        if ((e = lai_exec_process(state)))
            return e;
    }

    return 0;
}

static size_t lai_parse_varint(size_t *out, uint8_t *code, int *pc, int limit) {
    if (*pc + 1 > limit)
        return 1;
    uint8_t sz = (code[*pc] >> 6) & 3;
    if (!sz) {
        *out = (size_t)(code[*pc] & 0x3F);
        (*pc)++;
        return 0;
    } else if (sz == 1) {
        if (*pc + 2 > limit)
            return 1;
        *out = (size_t)(code[*pc] & 0x0F) | (size_t)(code[*pc + 1] << 4);
        *pc += 2;
        return 0;
    } else if (sz == 2) {
        if (*pc + 3 > limit)
            return 1;
        *out = (size_t)(code[*pc] & 0x0F) | (size_t)(code[*pc + 1] << 4)
               | (size_t)(code[*pc + 2] << 12);
        *pc += 3;
        return 0;
    } else {
        LAI_ENSURE(sz == 3);
        if (*pc + 4 > limit)
            return 1;
        *out = (size_t)(code[*pc] & 0x0F) | (size_t)(code[*pc + 1] << 4)
               | (size_t)(code[*pc + 2] << 12) | (size_t)(code[*pc + 3] << 20);
        *pc += 4;
        return 0;
    }
}

static int lai_parse_name(struct lai_amlname *out, uint8_t *code, int *pc, int limit) {
    (void)limit;
    *pc += lai_amlname_parse(out, code + *pc);
    return 0;
}

// Process the top-most item of the execution stack.
static lai_api_error_t lai_exec_process(lai_state_t *state) {
    lai_stackitem_t *item = lai_exec_peek_stack_back(state);
    struct lai_ctxitem *ctxitem = lai_exec_peek_ctxstack_back(state);
    struct lai_blkitem *block = lai_exec_peek_blkstack_back(state);
    LAI_ENSURE(ctxitem);
    LAI_ENSURE(block);
    struct lai_aml_segment *amls = ctxitem->amls;
    uint8_t *method = ctxitem->code;
    lai_nsnode_t *ctx_handle = ctxitem->handle;
    struct lai_invocation *invocation = ctxitem->invocation;

    // Package-size encoding (and similar) needs to know the PC of the opcode.
    // If an opcode sequence contains a pkgsize, the sequence generally ends at:
    //     opcode_pc + pkgsize + opcode size.
    int opcode_pc = block->pc;
    int limit = block->limit;

    // PC relative to the start of the table.
    // This matches the offsets in the output of 'iasl -l'.
    size_t table_pc = sizeof(acpi_header_t) + (method - amls->table->data) + opcode_pc;
    size_t table_limit_pc = sizeof(acpi_header_t) + (method - amls->table->data) + block->limit;

    // This would be an interpreter bug.
    if (block->pc > block->limit)
        lai_panic("execution escaped out of code range"
                  " [0x%lx, limit 0x%lx])",
                  table_pc, table_limit_pc);

    if (item->kind == LAI_POPULATE_STACKITEM) {
        if (block->pc == block->limit) {
            lai_exec_pop_blkstack_back(state);
            lai_exec_pop_ctxstack_back(state);
            lai_exec_pop_stack_back(state);
            return LAI_ERROR_NONE;
        } else {
            return lai_exec_parse(LAI_EXEC_MODE, state);
        }
    } else if (item->kind == LAI_METHOD_STACKITEM) {
        // ACPI does an implicit Return(0) at the end of a control method.
        if (block->pc == block->limit) {
            if (lai_exec_reserve_opstack(state))
                return LAI_ERROR_OUT_OF_MEMORY;

            if (state->opstack_ptr) // This is an internal error.
                lai_panic("opstack is not empty before return");
            if (item->mth_want_result) {
                struct lai_operand *result = lai_exec_push_opstack(state);
                result->tag = LAI_OPERAND_OBJECT;
                result->object.type = LAI_INTEGER;
                result->object.integer = 0;
            }

            // Clean up all per-method namespace nodes.
            struct lai_list_item *pmi;
            while ((pmi = lai_list_first(&invocation->per_method_list))) {
                lai_nsnode_t *node = LAI_CONTAINER_OF(pmi, lai_nsnode_t, per_method_item);

                if (node->type == LAI_NAMESPACE_BUFFER_FIELD) {
                    if (lai_rc_unref(&node->bf_buffer->rc)) {
                        laihost_free(node->bf_buffer->content, node->bf_buffer->size);
                        laihost_free(node->bf_buffer, sizeof(struct lai_buffer_head));
                    }
                }

                lai_uninstall_nsnode(node);
                lai_list_unlink(&node->per_method_item);
            }

            lai_exec_pop_blkstack_back(state);
            lai_exec_pop_ctxstack_back(state);
            lai_exec_pop_stack_back(state);
            return LAI_ERROR_NONE;
        } else {
            return lai_exec_parse(LAI_EXEC_MODE, state);
        }
    } else if (item->kind == LAI_BUFFER_STACKITEM) {
        int k = state->opstack_ptr - item->opstack_frame;
        LAI_ENSURE(k <= 1);
        if (k == 1) {
            LAI_CLEANUP_VAR lai_variable_t size = LAI_VAR_INITIALIZER;
            struct lai_operand *operand = lai_exec_get_opstack(state, item->opstack_frame);
            lai_exec_get_objectref(state, operand, &size);
            lai_exec_pop_opstack_back(state);

            // Note that not all elements of the buffer need to be initialized.
            LAI_CLEANUP_VAR lai_variable_t result = LAI_VAR_INITIALIZER;
            if (lai_create_buffer(&result, size.integer) != LAI_ERROR_NONE)
                lai_panic("failed to allocate memory for AML buffer");

            int initial_size = block->limit - block->pc;
            if (initial_size < 0)
                lai_panic("buffer initializer has negative size");
            if (initial_size > (int)lai_exec_buffer_size(&result))
                lai_panic("buffer initializer overflows buffer");
            memcpy(lai_exec_buffer_access(&result), method + block->pc, initial_size);

            if (item->buf_want_result) {
                // Note: there is no need to reserve() as we pop an operand above.
                struct lai_operand *opstack_res = lai_exec_push_opstack(state);
                opstack_res->tag = LAI_OPERAND_OBJECT;
                lai_var_move(&opstack_res->object, &result);
            }

            lai_exec_pop_blkstack_back(state);
            lai_exec_pop_stack_back(state);
            return LAI_ERROR_NONE;
        } else {
            return lai_exec_parse(LAI_OBJECT_MODE, state);
        }
    } else if (item->kind == LAI_PACKAGE_STACKITEM || item->kind == LAI_VARPACKAGE_STACKITEM) {
        struct lai_operand *frame = lai_exec_get_opstack(state, item->opstack_frame);
        if (item->pkg_phase == 0) {
            lai_api_error_t error = LAI_ERROR_NONE;
            if (item->kind == LAI_PACKAGE_STACKITEM)
                error = lai_exec_parse(LAI_IMMEDIATE_BYTE_MODE, state);
            else
                error = lai_exec_parse(LAI_OBJECT_MODE, state);

            item->pkg_phase++;

            return error;
        } else if (item->pkg_phase == 1) {
            LAI_CLEANUP_VAR lai_variable_t size = LAI_VAR_INITIALIZER;
            LAI_TRY(lai_exec_get_integer(state, &frame[1], &size));

            lai_exec_pop_opstack_back(state);

            if (lai_create_pkg(&frame[0].object, size.integer) != LAI_ERROR_NONE)
                lai_panic("could not allocate memory for package");

            item->pkg_phase++;

            return LAI_ERROR_NONE;
        }

        if (state->opstack_ptr == item->opstack_frame + 2) {
            struct lai_operand *package = &frame[0];
            LAI_ENSURE(package->tag == LAI_OPERAND_OBJECT);
            struct lai_operand *initializer = &frame[1];
            LAI_ENSURE(initializer->tag == LAI_OPERAND_OBJECT);

            if (item->pkg_index == (int)lai_exec_pkg_size(&package->object))
                lai_panic("package initializer overflows its size");
            LAI_ENSURE(item->pkg_index < (int)lai_exec_pkg_size(&package->object));

            lai_exec_pkg_store(&initializer->object, &package->object, item->pkg_index);
            item->pkg_index++;
            lai_exec_pop_opstack_back(state);
        }
        LAI_ENSURE(state->opstack_ptr == item->opstack_frame + 1);

        if (block->pc == block->limit) {
            if (!item->pkg_want_result)
                lai_exec_pop_opstack_back(state);

            lai_exec_pop_blkstack_back(state);
            lai_exec_pop_stack_back(state);
            return LAI_ERROR_NONE;
        } else {
            return lai_exec_parse(LAI_DATA_MODE, state);
        }
    } else if (item->kind == LAI_NODE_STACKITEM) {
        int k = state->opstack_ptr - item->opstack_frame;
        if (!item->node_arg_modes[k]) {
            struct lai_operand *operands = lai_exec_get_opstack(state, item->opstack_frame);
            lai_exec_reduce_node(item->node_opcode, state, operands, ctx_handle);
            lai_exec_pop_opstack(state, k);

            lai_exec_pop_stack_back(state);
            return LAI_ERROR_NONE;
        } else {
            return lai_exec_parse(item->node_arg_modes[k], state);
        }
    } else if (item->kind == LAI_OP_STACKITEM) {
        int k = state->opstack_ptr - item->opstack_frame;
        //            lai_debug("got %d parameters", k);
        if (!item->op_arg_modes[k]) {
            if (lai_exec_reserve_opstack(state))
                return LAI_ERROR_OUT_OF_MEMORY;

            lai_variable_t result = {0};
            struct lai_operand *operands = lai_exec_get_opstack(state, item->opstack_frame);
            lai_api_error_t error = lai_exec_reduce_op(item->op_opcode, state, operands, &result);
            if (error != LAI_ERROR_NONE) {
                return error;
            }
            lai_exec_pop_opstack(state, k);

            if (item->op_want_result) {
                struct lai_operand *opstack_res = lai_exec_push_opstack(state);
                opstack_res->tag = LAI_OPERAND_OBJECT;
                lai_var_move(&opstack_res->object, &result);
            } else {
                lai_var_finalize(&result);
            }

            lai_exec_pop_stack_back(state);
            return LAI_ERROR_NONE;
        } else {
            return lai_exec_parse(item->op_arg_modes[k], state);
        }
    } else if (item->kind == LAI_INVOKE_STACKITEM) {
        int argc = item->ivk_argc;
        int want_result = item->ivk_want_result;
        int k = state->opstack_ptr - item->opstack_frame;
        LAI_ENSURE(k <= argc + 1);
        if (k == argc + 1) { // First operand is the method name.
            if (lai_exec_reserve_ctxstack(state) || lai_exec_reserve_blkstack(state))
                return LAI_ERROR_OUT_OF_MEMORY;

            struct lai_operand *opstack_method = lai_exec_get_opstack(state, item->opstack_frame);
            LAI_ENSURE(opstack_method->tag == LAI_RESOLVED_NAME);

            lai_nsnode_t *handle = opstack_method->handle;
            LAI_ENSURE(handle->type == LAI_NAMESPACE_METHOD);

            // TODO: Make sure that this does not leak memory.
            lai_variable_t args[7];
            memset(args, 0, sizeof(lai_variable_t) * 7);

            for (int i = 0; i < argc; i++) {
                struct lai_operand *operand
                    = lai_exec_get_opstack(state, item->opstack_frame + 1 + i);
                lai_exec_get_objectref(state, operand, &args[i]);
            }

            lai_exec_pop_opstack(state, argc + 1);
            lai_exec_pop_stack_back(state);

            if (handle->method_override) {
                // It's an OS-defined method.
                // TODO: Verify the number of argument to the overridden method.
                LAI_CLEANUP_VAR lai_variable_t method_result = LAI_VAR_INITIALIZER;
                int e = handle->method_override(args, &method_result);

                if (e) {
                    lai_warn("overriden control method failed");
                    return LAI_ERROR_EXECUTION_FAILURE;
                }
                if (want_result) {
                    // Note: there is no need to reserve() as we pop an operand above.
                    struct lai_operand *opstack_res = lai_exec_push_opstack(state);
                    opstack_res->tag = LAI_OPERAND_OBJECT;
                    lai_var_move(&opstack_res->object, &method_result);
                }
            } else {
                // It's an AML method.
                LAI_ENSURE(handle->amls);

                struct lai_ctxitem *method_ctxitem = lai_exec_push_ctxstack(state);
                method_ctxitem->amls = handle->amls;
                method_ctxitem->code = handle->pointer;
                method_ctxitem->handle = handle;
                method_ctxitem->invocation = laihost_malloc(sizeof(struct lai_invocation));
                if (!method_ctxitem->invocation)
                    lai_panic("could not allocate memory for method invocation");
                memset(method_ctxitem->invocation, 0, sizeof(struct lai_invocation));
                lai_list_init(&method_ctxitem->invocation->per_method_list);

                for (int i = 0; i < argc; i++)
                    lai_var_move(&method_ctxitem->invocation->arg[i], &args[i]);

                struct lai_blkitem *blkitem = lai_exec_push_blkstack(state);
                blkitem->pc = 0;
                blkitem->limit = handle->size;

                // Note: there is no need to reserve() as we pop a stackitem above.
                lai_stackitem_t *item = lai_exec_push_stack(state);
                item->kind = LAI_METHOD_STACKITEM;
                item->mth_want_result = want_result;
            }
            return LAI_ERROR_NONE;
        } else {
            return lai_exec_parse(LAI_OBJECT_MODE, state);
        }
    } else if (item->kind == LAI_RETURN_STACKITEM) {
        int k = state->opstack_ptr - item->opstack_frame;
        LAI_ENSURE(k <= 1);
        if (k == 1) {
            LAI_CLEANUP_VAR lai_variable_t result = LAI_VAR_INITIALIZER;
            struct lai_operand *operand = lai_exec_get_opstack(state, item->opstack_frame);
            lai_exec_get_objectref(state, operand, &result);
            lai_exec_pop_opstack_back(state);

            // Find the last LAI_METHOD_STACKITEM on the stack.
            int m = 0;
            lai_stackitem_t *method_item;
            while (1) {
                // Ignore the top-most LAI_RETURN_STACKITEM.
                method_item = lai_exec_peek_stack(state, 1 + m);
                if (!method_item)
                    lai_panic("Return() outside of control method()");
                if (method_item->kind == LAI_METHOD_STACKITEM)
                    break;
                if (method_item->kind != LAI_COND_STACKITEM
                    && method_item->kind != LAI_LOOP_STACKITEM)
                    lai_panic("Return() cannot skip item of type %d", method_item->kind);
                m++;
            }

            // Push the return value.
            if (method_item->mth_want_result) {
                // Note: there is no need to reserve() as we pop an operand above.
                struct lai_operand *opstack_res = lai_exec_push_opstack(state);
                opstack_res->tag = LAI_OPERAND_OBJECT;
                lai_obj_clone(&opstack_res->object, &result);
            }

            // Clean up all per-method namespace nodes.
            struct lai_list_item *pmi;
            while ((pmi = lai_list_first(&invocation->per_method_list))) {
                lai_nsnode_t *node = LAI_CONTAINER_OF(pmi, lai_nsnode_t, per_method_item);

                if (node->type == LAI_NAMESPACE_BUFFER_FIELD) {
                    if (lai_rc_unref(&node->bf_buffer->rc)) {
                        laihost_free(node->bf_buffer->content, node->bf_buffer->size);
                        laihost_free(node->bf_buffer, sizeof(struct lai_buffer_head));
                    }
                }

                lai_uninstall_nsnode(node);
                lai_list_unlink(&node->per_method_item);
            }

            // Pop the LAI_RETURN_STACKITEM.
            lai_exec_pop_stack_back(state);

            // Pop all nested loops/conditions.
            for (int i = 0; i < m; i++) {
                lai_stackitem_t *pop_item = lai_exec_peek_stack_back(state);
                LAI_ENSURE(pop_item->kind == LAI_COND_STACKITEM
                           || pop_item->kind == LAI_LOOP_STACKITEM);
                lai_exec_pop_blkstack_back(state);
                lai_exec_pop_stack_back(state);
            }

            // Pop the LAI_METHOD_STACKITEM.
            lai_exec_pop_ctxstack_back(state);
            lai_exec_pop_blkstack_back(state);
            lai_exec_pop_stack_back(state);
            return LAI_ERROR_NONE;
        } else {
            return lai_exec_parse(LAI_OBJECT_MODE, state);
        }
    } else if (item->kind == LAI_LOOP_STACKITEM) {
        if (!item->loop_state) {
            // We are at the beginning of a loop and need to check the predicate.
            int k = state->opstack_ptr - item->opstack_frame;
            LAI_ENSURE(k <= 1);
            if (k == 1) {
                LAI_CLEANUP_VAR lai_variable_t predicate = LAI_VAR_INITIALIZER;
                struct lai_operand *operand = lai_exec_get_opstack(state, item->opstack_frame);
                LAI_TRY(lai_exec_get_integer(state, operand, &predicate));
                lai_exec_pop_opstack_back(state);

                if (predicate.integer) {
                    item->loop_state = LAI_LOOP_ITERATION;
                } else {
                    lai_exec_pop_blkstack_back(state);
                    lai_exec_pop_stack_back(state);
                }
                return LAI_ERROR_NONE;
            } else {
                return lai_exec_parse(LAI_OBJECT_MODE, state);
            }
        } else {
            LAI_ENSURE(item->loop_state == LAI_LOOP_ITERATION);
            // Unconditionally reset the loop's state to recheck the predicate.
            if (block->pc == block->limit) {
                item->loop_state = 0;
                block->pc = item->loop_pred;
                return LAI_ERROR_NONE;
            } else {
                return lai_exec_parse(LAI_EXEC_MODE, state);
            }
        }
    } else if (item->kind == LAI_COND_STACKITEM) {
        if (!item->cond_state) {
            // We are at the beginning of the condition and need to check the predicate.
            int k = state->opstack_ptr - item->opstack_frame;
            LAI_ENSURE(k <= 1);
            if (k == 1) {
                LAI_CLEANUP_VAR lai_variable_t predicate = LAI_VAR_INITIALIZER;
                struct lai_operand *operand = lai_exec_get_opstack(state, item->opstack_frame);
                LAI_TRY(lai_exec_get_integer(state, operand, &predicate));
                lai_exec_pop_opstack_back(state);

                if (predicate.integer) {
                    item->cond_state = LAI_COND_BRANCH;
                } else {
                    if (item->cond_has_else) {
                        item->cond_state = LAI_COND_BRANCH;
                        block->pc = item->cond_else_pc;
                        block->limit = item->cond_else_limit;
                    } else {
                        lai_exec_pop_blkstack_back(state);
                        lai_exec_pop_stack_back(state);
                    }
                }
                return LAI_ERROR_NONE;
            } else {
                return lai_exec_parse(LAI_OBJECT_MODE, state);
            }
        } else {
            LAI_ENSURE(item->cond_state == LAI_COND_BRANCH);
            if (block->pc == block->limit) {
                lai_exec_pop_blkstack_back(state);
                lai_exec_pop_stack_back(state);
                return LAI_ERROR_NONE;
            } else {
                return lai_exec_parse(LAI_EXEC_MODE, state);
            }
        }
    } else if (item->kind == LAI_BANKFIELD_STACKITEM) {
        int k = state->opstack_ptr - item->opstack_frame;
        LAI_ENSURE(k <= 3);
        if (k == 3) { // there's already region_name and bank_name on there
            LAI_CLEANUP_VAR lai_variable_t bank_value_var = LAI_VAR_INITIALIZER;
            struct lai_operand *operand;

            operand = lai_exec_get_opstack(state, item->opstack_frame);
            lai_nsnode_t *region_node = operand->handle;

            operand = lai_exec_get_opstack(state, item->opstack_frame + 1);
            lai_nsnode_t *bank_node = operand->handle;

            operand = lai_exec_get_opstack(state, item->opstack_frame + 2);
            LAI_TRY(lai_exec_get_integer(state, operand, &bank_value_var));
            uint64_t bank_value = bank_value_var.integer;

            lai_exec_pop_opstack(state, 3);

            int pc = block->pc;

            uint8_t access_type = *(method + pc);
            pc++;

            // parse FieldList
            struct lai_amlname field_amln;
            uint64_t curr_off = 0;
            size_t skip_bits;
            while (pc < block->limit) {
                switch (*(method + pc)) {
                    case 0: // ReservedField
                        pc++;
                        // TODO: Partially failing to parse a Field() is a bad idea.
                        if (lai_parse_varint(&skip_bits, method, &pc, limit))
                            return LAI_ERROR_EXECUTION_FAILURE;
                        curr_off += skip_bits;
                        break;
                    case 1: // AccessField
                        pc++;
                        access_type = *(method + pc);
                        pc += 2;
                        break;
                    case 2: // TODO: ConnectField
                        lai_panic("ConnectField parsing isn't implemented");
                        break;
                    default: // NamedField
                        // TODO: Partially failing to parse a Field() is a bad idea.
                        if (lai_parse_name(&field_amln, method, &pc, limit)
                            || lai_parse_varint(&skip_bits, method, &pc, limit))
                            return LAI_ERROR_EXECUTION_FAILURE;

                        lai_nsnode_t *node = lai_create_nsnode_or_die();
                        node->type = LAI_NAMESPACE_BANKFIELD;
                        node->fld_region_node = region_node;
                        node->fld_flags = access_type;
                        node->fld_size = skip_bits;
                        node->fld_offset = curr_off;

                        node->fld_bkf_bank_node = bank_node;
                        node->fld_bkf_value = bank_value;
                        lai_do_resolve_new_node(node, ctx_handle, &field_amln);
                        LAI_TRY(lai_install_nsnode(node));

                        if (invocation)
                            lai_list_link(&invocation->per_method_list, &node->per_method_item);

                        curr_off += skip_bits;
                }
            }

            lai_exec_pop_blkstack_back(state);
            lai_exec_pop_stack_back(state);
            return LAI_ERROR_NONE;
        } else {
            return lai_exec_parse(LAI_OBJECT_MODE, state);
        }
    } else
        lai_panic("unexpected lai_stackitem_t");
}

static inline int lai_parse_u8(uint8_t *out, uint8_t *code, int *pc, int limit) {
    if (*pc + 1 > limit)
        return 1;
    *out = code[*pc];
    (*pc)++;
    return 0;
}

static inline int lai_parse_u16(uint16_t *out, uint8_t *code, int *pc, int limit) {
    if (*pc + 2 > limit)
        return 1;
    *out = ((uint16_t)code[*pc]) | (((uint16_t)code[*pc + 1]) << 8);
    *pc += 2;
    return 0;
}

static inline int lai_parse_u32(uint32_t *out, uint8_t *code, int *pc, int limit) {
    if (*pc + 4 > limit)
        return 1;
    *out = ((uint32_t)code[*pc]) | (((uint32_t)code[*pc + 1]) << 8)
           | (((uint32_t)code[*pc + 2]) << 16) | (((uint32_t)code[*pc + 3]) << 24);
    *pc += 4;
    return 0;
}

static inline int lai_parse_u64(uint64_t *out, uint8_t *code, int *pc, int limit) {
    if (*pc + 8 > limit)
        return 1;
    *out = ((uint64_t)code[*pc]) | (((uint64_t)code[*pc + 1]) << 8)
           | (((uint64_t)code[*pc + 2]) << 16) | (((uint64_t)code[*pc + 3]) << 24)
           | (((uint64_t)code[*pc + 4]) << 32) | (((uint64_t)code[*pc + 5]) << 40)
           | (((uint64_t)code[*pc + 6]) << 48) | (((uint64_t)code[*pc + 7]) << 56);
    *pc += 8;
    return 0;
}

// Advances the PC of the current block.
// lai_exec_parse() calls this function after successfully parsing a full opcode.
// Even if parsing fails, this mechanism makes sure that the PC never points to
// the middle of an opcode.
static inline void lai_exec_commit_pc(lai_state_t *state, int pc) {
    // Note that we re-read the block pointer, as the block stack might have been reallocated.
    struct lai_blkitem *block = lai_exec_peek_blkstack_back(state);
    LAI_ENSURE(block);
    block->pc = pc;
}

static lai_api_error_t lai_exec_parse(int parse_mode, lai_state_t *state) {
    struct lai_ctxitem *ctxitem = lai_exec_peek_ctxstack_back(state);
    struct lai_blkitem *block = lai_exec_peek_blkstack_back(state);
    LAI_ENSURE(ctxitem);
    LAI_ENSURE(block);
    struct lai_aml_segment *amls = ctxitem->amls;
    uint8_t *method = ctxitem->code;
    lai_nsnode_t *ctx_handle = ctxitem->handle;
    struct lai_invocation *invocation = ctxitem->invocation;
    struct lai_instance *instance = lai_current_instance();

    int pc = block->pc;
    int limit = block->limit;

    // Package-size encoding (and similar) needs to know the PC of the opcode.
    // If an opcode sequence contains a pkgsize, the sequence generally ends at:
    //     opcode_pc + pkgsize + opcode size.
    int opcode_pc = pc;

    // PC relative to the start of the table.
    // This matches the offsets in the output of 'iasl -l'.
    size_t table_pc = sizeof(acpi_header_t) + (method - amls->table->data) + opcode_pc;
    size_t table_limit_pc = sizeof(acpi_header_t) + (method - amls->table->data) + block->limit;

    if (!(pc < block->limit))
        lai_panic("execution escaped out of code range"
                  " [0x%lx, limit 0x%lx])",
                  table_pc, table_limit_pc);

    // Whether we use the result of an expression or not.
    // If yes, it will be pushed onto the opstack after the expression is computed.
    int want_result = lai_mode_flags[parse_mode] & LAI_MF_RESULT;

    if (parse_mode == LAI_IMMEDIATE_BYTE_MODE) {
        uint8_t value = 0;
        if (lai_parse_u8(&value, method, &pc, limit))
            return LAI_ERROR_EXECUTION_FAILURE;

        if (lai_exec_reserve_opstack(state))
            return LAI_ERROR_OUT_OF_MEMORY;
        lai_exec_commit_pc(state, pc);

        struct lai_operand *result = lai_exec_push_opstack(state);
        result->tag = LAI_OPERAND_OBJECT;
        result->object.type = LAI_INTEGER;
        result->object.integer = value;
        return LAI_ERROR_NONE;
    } else if (parse_mode == LAI_IMMEDIATE_WORD_MODE) {
        uint16_t value = 0;
        if (lai_parse_u16(&value, method, &pc, limit))
            return LAI_ERROR_EXECUTION_FAILURE;

        if (lai_exec_reserve_opstack(state))
            return LAI_ERROR_OUT_OF_MEMORY;
        lai_exec_commit_pc(state, pc);

        struct lai_operand *result = lai_exec_push_opstack(state);
        result->tag = LAI_OPERAND_OBJECT;
        result->object.type = LAI_INTEGER;
        result->object.integer = value;
        return LAI_ERROR_NONE;
    } else if (parse_mode == LAI_IMMEDIATE_DWORD_MODE) {
        uint32_t value = 0;
        if (lai_parse_u32(&value, method, &pc, limit))
            return LAI_ERROR_EXECUTION_FAILURE;

        if (lai_exec_reserve_opstack(state))
            return LAI_ERROR_OUT_OF_MEMORY;
        lai_exec_commit_pc(state, pc);

        struct lai_operand *result = lai_exec_push_opstack(state);
        result->tag = LAI_OPERAND_OBJECT;
        result->object.type = LAI_INTEGER;
        result->object.integer = value;
        return LAI_ERROR_NONE;
    }

    // Process names.
    if (lai_is_name(method[pc])) {
        struct lai_amlname amln;
        if (lai_parse_name(&amln, method, &pc, limit))
            return LAI_ERROR_EXECUTION_FAILURE;

        if (lai_exec_reserve_opstack(state) || lai_exec_reserve_stack(state))
            return LAI_ERROR_OUT_OF_MEMORY;
        lai_exec_commit_pc(state, pc);

        LAI_CLEANUP_FREE_STRING char *path = NULL;
        if (instance->trace & LAI_TRACE_OP)
            path = lai_stringify_amlname(&amln);

        if (parse_mode == LAI_DATA_MODE) {
            if (instance->trace & LAI_TRACE_OP)
                lai_debug("parsing name %s [@ 0x%lx]", path, table_pc);

            if (want_result) {
                struct lai_operand *opstack_res = lai_exec_push_opstack(state);
                opstack_res->tag = LAI_OPERAND_OBJECT;
                opstack_res->object.type = LAI_LAZY_HANDLE;
                opstack_res->object.unres_ctx_handle = ctx_handle;
                opstack_res->object.unres_aml = method + opcode_pc;
            }
        } else if (!(lai_mode_flags[parse_mode] & LAI_MF_RESOLVE)) {
            if (instance->trace & LAI_TRACE_OP)
                lai_debug("parsing name %s [@ 0x%lx]", path, table_pc);

            if (want_result) {
                struct lai_operand *opstack_res = lai_exec_push_opstack(state);
                opstack_res->tag = LAI_UNRESOLVED_NAME;
                opstack_res->unres_ctx_handle = ctx_handle;
                opstack_res->unres_aml = method + opcode_pc;
            }
        } else {
            lai_nsnode_t *handle = lai_do_resolve(ctx_handle, &amln);
            if (!handle) {
                if (lai_mode_flags[parse_mode] & LAI_MF_NULLABLE) {
                    if (instance->trace & LAI_TRACE_OP)
                        lai_debug("parsing non-existant name %s [@ 0x%lx]", path, table_pc);

                    if (want_result) {
                        struct lai_operand *opstack_res = lai_exec_push_opstack(state);
                        opstack_res->tag = LAI_RESOLVED_NAME;
                        opstack_res->handle = NULL;
                    }
                } else {
                    lai_warn("undefined reference %s in object mode, aborting",
                             lai_stringify_amlname(&amln));
                    return LAI_ERROR_UNEXPECTED_RESULT;
                }
            } else if (handle->type == LAI_NAMESPACE_METHOD
                       && (lai_mode_flags[parse_mode] & LAI_MF_INVOKE)) {
                if (instance->trace & LAI_TRACE_OP)
                    lai_debug("parsing invocation %s [@ 0x%lx]", path, table_pc);

                lai_stackitem_t *node_item = lai_exec_push_stack(state);
                node_item->kind = LAI_INVOKE_STACKITEM;
                node_item->opstack_frame = state->opstack_ptr;
                node_item->ivk_argc = handle->method_flags & METHOD_ARGC_MASK;
                node_item->ivk_want_result = want_result;

                struct lai_operand *opstack_method = lai_exec_push_opstack(state);
                opstack_method->tag = LAI_RESOLVED_NAME;
                opstack_method->handle = handle;
            } else if (lai_mode_flags[parse_mode] & LAI_MF_INVOKE) {
                // TODO: Get rid of this case again!
                if (instance->trace & LAI_TRACE_OP)
                    lai_debug("parsing name %s [@ 0x%lx]", path, table_pc);

                LAI_CLEANUP_VAR lai_variable_t result = LAI_VAR_INITIALIZER;
                lai_exec_access(&result, handle);

                if (want_result) {
                    struct lai_operand *opstack_res = lai_exec_push_opstack(state);
                    opstack_res->tag = LAI_OPERAND_OBJECT;
                    lai_var_move(&opstack_res->object, &result);
                }
            } else {
                if (instance->trace & LAI_TRACE_OP)
                    lai_debug("parsing name %s [@ 0x%lx]", path, table_pc);

                if (want_result) {
                    struct lai_operand *opstack_method = lai_exec_push_opstack(state);
                    opstack_method->tag = LAI_RESOLVED_NAME;
                    opstack_method->handle = handle;
                }
            }
        }
        return LAI_ERROR_NONE;
    }

    /* General opcodes */
    int opcode;
    if (method[pc] == EXTOP_PREFIX) {
        if (pc + 1 == block->limit)
            lai_panic("two-byte opcode on method boundary");
        opcode = (EXTOP_PREFIX << 8) | method[pc + 1];
        pc += 2;
    } else {
        opcode = method[pc];
        pc++;
    }
    if (instance->trace & LAI_TRACE_OP) {
        lai_debug("parsing opcode 0x%02x [0x%lx @ %c%c%c%c %ld]", opcode, table_pc,
                  amls->table->header.signature[0], amls->table->header.signature[1],
                  amls->table->header.signature[2], amls->table->header.signature[3], amls->index);
    }

    // This switch handles the majority of all opcodes.
    switch (opcode) {
        case NOP_OP:
            lai_exec_commit_pc(state, pc);
            break;

        case ZERO_OP:
            if (lai_exec_reserve_opstack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            if (parse_mode == LAI_DATA_MODE || parse_mode == LAI_OBJECT_MODE) {
                struct lai_operand *result = lai_exec_push_opstack(state);
                result->tag = LAI_OPERAND_OBJECT;
                result->object.type = LAI_INTEGER;
                result->object.integer = 0;
            } else if (parse_mode == LAI_REFERENCE_MODE
                       || parse_mode == LAI_OPTIONAL_REFERENCE_MODE) {
                // In target mode, ZERO_OP generates a null target and not an integer!
                struct lai_operand *result = lai_exec_push_opstack(state);
                result->tag = LAI_NULL_NAME;
            } else {
                lai_warn("Zero() in execution mode has no effect");
                LAI_ENSURE(parse_mode == LAI_EXEC_MODE);
            }
            break;
        case ONE_OP:
            if (lai_exec_reserve_opstack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            if (parse_mode == LAI_DATA_MODE || parse_mode == LAI_OBJECT_MODE) {
                struct lai_operand *result = lai_exec_push_opstack(state);
                result->tag = LAI_OPERAND_OBJECT;
                result->object.type = LAI_INTEGER;
                result->object.integer = 1;
            } else {
                lai_warn("One() in execution mode has no effect");
                LAI_ENSURE(parse_mode == LAI_EXEC_MODE);
            }
            break;
        case ONES_OP:
            if (lai_exec_reserve_opstack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            if (parse_mode == LAI_DATA_MODE || parse_mode == LAI_OBJECT_MODE) {
                struct lai_operand *result = lai_exec_push_opstack(state);
                result->tag = LAI_OPERAND_OBJECT;
                result->object.type = LAI_INTEGER;
                result->object.integer = ~((uint64_t)0);
            } else {
                lai_warn("Ones() in execution mode has no effect");
                LAI_ENSURE(parse_mode == LAI_EXEC_MODE);
            }
            break;
        case (EXTOP_PREFIX << 8) | REVISION_OP:
            if (lai_exec_reserve_opstack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            if (parse_mode == LAI_DATA_MODE || parse_mode == LAI_OBJECT_MODE) {
                struct lai_operand *result = lai_exec_push_opstack(state);
                result->tag = LAI_OPERAND_OBJECT;
                result->object.type = LAI_INTEGER;
                result->object.integer = LAI_REVISION;
            } else {
                lai_warn("Revision() in execution mode has no effect");
                LAI_ENSURE(parse_mode == LAI_EXEC_MODE);
            }
            break;

        case (EXTOP_PREFIX << 8) | TIMER_OP:
            if (lai_exec_reserve_opstack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            if (parse_mode == LAI_DATA_MODE || parse_mode == LAI_OBJECT_MODE) {
                if (!laihost_timer)
                    lai_panic("host does not provide timer functions required by Timer()");

                struct lai_operand *result = lai_exec_push_opstack(state);
                result->tag = LAI_OPERAND_OBJECT;
                result->object.type = LAI_INTEGER;
                result->object.integer = laihost_timer();
            } else {
                lai_warn("Timer() in execution mode has no effect");
                LAI_ENSURE(parse_mode == LAI_EXEC_MODE);
            }
            break;

        case BYTEPREFIX:
        case WORDPREFIX:
        case DWORDPREFIX:
        case QWORDPREFIX: {
            uint64_t value = 0;
            switch (opcode) {
                case BYTEPREFIX: {
                    uint8_t temp;
                    if (lai_parse_u8(&temp, method, &pc, limit))
                        return LAI_ERROR_EXECUTION_FAILURE;
                    value = temp;
                    break;
                }
                case WORDPREFIX: {
                    uint16_t temp;
                    if (lai_parse_u16(&temp, method, &pc, limit))
                        return LAI_ERROR_EXECUTION_FAILURE;
                    value = temp;
                    break;
                }
                case DWORDPREFIX: {
                    uint32_t temp;
                    if (lai_parse_u32(&temp, method, &pc, limit))
                        return LAI_ERROR_EXECUTION_FAILURE;
                    value = temp;
                    break;
                }
                case QWORDPREFIX: {
                    if (lai_parse_u64(&value, method, &pc, limit))
                        return LAI_ERROR_EXECUTION_FAILURE;
                    break;
                }
            }

            if (lai_exec_reserve_opstack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            if (parse_mode == LAI_DATA_MODE || parse_mode == LAI_OBJECT_MODE) {
                struct lai_operand *result = lai_exec_push_opstack(state);
                result->tag = LAI_OPERAND_OBJECT;
                result->object.type = LAI_INTEGER;
                result->object.integer = value;
            } else
                LAI_ENSURE(parse_mode == LAI_EXEC_MODE);
            break;
        }
        case STRINGPREFIX: {
            int data_pc;
            size_t n = 0; // Length of null-terminated string.
            while (pc + n < (size_t)block->limit && method[pc + n])
                n++;
            if (pc + n == (size_t)block->limit)
                lai_panic("unterminated string in AML code");
            data_pc = pc;
            pc += n + 1;

            if (lai_exec_reserve_opstack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            if (parse_mode == LAI_DATA_MODE || parse_mode == LAI_OBJECT_MODE) {
                struct lai_operand *opstack_res = lai_exec_push_opstack(state);
                opstack_res->tag = LAI_OPERAND_OBJECT;
                if (lai_create_string(&opstack_res->object, n) != LAI_ERROR_NONE)
                    lai_panic("could not allocate memory for string");
                memcpy(lai_exec_string_access(&opstack_res->object), method + data_pc, n);
            } else
                LAI_ENSURE(parse_mode == LAI_EXEC_MODE);
            break;
        }
        case BUFFER_OP: {
            int data_pc;
            size_t encoded_size; // Size of the buffer initializer.
            if (lai_parse_varint(&encoded_size, method, &pc, limit))
                return LAI_ERROR_EXECUTION_FAILURE;
            data_pc = pc;
            pc = opcode_pc + 1 + encoded_size;

            if (lai_exec_reserve_blkstack(state) || lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            struct lai_blkitem *blkitem = lai_exec_push_blkstack(state);
            blkitem->pc = data_pc;
            blkitem->limit = opcode_pc + 1 + encoded_size;

            lai_stackitem_t *buf_item = lai_exec_push_stack(state);
            buf_item->kind = LAI_BUFFER_STACKITEM;
            buf_item->opstack_frame = state->opstack_ptr;
            buf_item->buf_want_result = want_result;
            break;
        }
        case VARPACKAGE_OP: {
            int data_pc;
            size_t encoded_size; // Size of the package initializer.
            if (lai_parse_varint(&encoded_size, method, &pc, limit))
                return LAI_ERROR_EXECUTION_FAILURE;
            data_pc = pc;
            pc = opcode_pc + 1 + encoded_size;

            if (lai_exec_reserve_opstack(state) || lai_exec_reserve_blkstack(state)
                || lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            // Note that not all elements of the package need to be initialized.

            struct lai_blkitem *blkitem = lai_exec_push_blkstack(state);
            blkitem->pc = data_pc;
            blkitem->limit = opcode_pc + 1 + encoded_size;

            lai_stackitem_t *pkg_item = lai_exec_push_stack(state);
            pkg_item->kind = LAI_VARPACKAGE_STACKITEM;
            pkg_item->opstack_frame = state->opstack_ptr;
            pkg_item->pkg_index = 0;
            pkg_item->pkg_want_result = want_result;
            pkg_item->pkg_phase = 0;

            struct lai_operand *opstack_pkg = lai_exec_push_opstack(state);
            opstack_pkg->tag = LAI_OPERAND_OBJECT;
            break;
        }
        case PACKAGE_OP: {
            int data_pc;
            size_t encoded_size; // Size of the package initializer.
            if (lai_parse_varint(&encoded_size, method, &pc, limit))
                return LAI_ERROR_EXECUTION_FAILURE;
            data_pc = pc;
            pc = opcode_pc + 1 + encoded_size;

            if (lai_exec_reserve_opstack(state) || lai_exec_reserve_blkstack(state)
                || lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            // Note that not all elements of the package need to be initialized.

            struct lai_blkitem *blkitem = lai_exec_push_blkstack(state);
            blkitem->pc = data_pc;
            blkitem->limit = opcode_pc + 1 + encoded_size;

            lai_stackitem_t *pkg_item = lai_exec_push_stack(state);
            pkg_item->kind = LAI_PACKAGE_STACKITEM;
            pkg_item->opstack_frame = state->opstack_ptr;
            pkg_item->pkg_index = 0;
            pkg_item->pkg_want_result = want_result;
            pkg_item->pkg_phase = 0;

            struct lai_operand *opstack_pkg = lai_exec_push_opstack(state);
            opstack_pkg->tag = LAI_OPERAND_OBJECT;
            break;
        }

        /* A control method can return literally any object */
        /* So we need to take this into consideration */
        case RETURN_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *node_item = lai_exec_push_stack(state);
            node_item->kind = LAI_RETURN_STACKITEM;
            node_item->opstack_frame = state->opstack_ptr;
            break;
        }
        /* While Loops */
        case WHILE_OP: {
            int body_pc;
            size_t loop_size;
            if (lai_parse_varint(&loop_size, method, &pc, limit))
                return LAI_ERROR_EXECUTION_FAILURE;
            body_pc = pc;
            pc = opcode_pc + 1 + loop_size;

            if (lai_exec_reserve_blkstack(state) || lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            struct lai_blkitem *blkitem = lai_exec_push_blkstack(state);
            blkitem->pc = body_pc;
            blkitem->limit = opcode_pc + 1 + loop_size;

            lai_stackitem_t *loop_item = lai_exec_push_stack(state);
            loop_item->kind = LAI_LOOP_STACKITEM;
            loop_item->opstack_frame = state->opstack_ptr;
            loop_item->loop_state = 0;
            loop_item->loop_pred = body_pc;
            break;
        }
        /* Continue Looping */
        case CONTINUE_OP: {
            // Find the last LAI_LOOP_STACKITEM on the stack.
            int m = 0;
            lai_stackitem_t *loop_item;
            while (1) {
                loop_item = lai_exec_peek_stack(state, m);
                if (!loop_item)
                    lai_panic("Continue() outside of While()");
                if (loop_item->kind == LAI_LOOP_STACKITEM)
                    break;
                if (loop_item->kind != LAI_COND_STACKITEM && loop_item->kind != LAI_LOOP_STACKITEM)
                    lai_panic("Continue() cannot skip item of type %d", loop_item->kind);
                m++;
            }

            // Pop all nested loops/conditions.
            for (int i = 0; i < m; i++) {
                lai_stackitem_t *pop_item = lai_exec_peek_stack_back(state);
                LAI_ENSURE(pop_item->kind == LAI_COND_STACKITEM
                           || pop_item->kind == LAI_LOOP_STACKITEM);
                lai_exec_pop_blkstack_back(state);
                lai_exec_pop_stack_back(state);
            }

            // Set the PC of the block to its limit, to trigger a recheck of the predicate.
            lai_exec_peek_blkstack_back(state)->pc = lai_exec_peek_blkstack_back(state)->limit;
            break;
        }
        /* Break Loop */
        case BREAK_OP: {
            // Find the last LAI_LOOP_STACKITEM on the stack.
            int m = 0;
            lai_stackitem_t *loop_item;
            while (1) {
                loop_item = lai_exec_peek_stack(state, m);
                if (!loop_item)
                    lai_panic("Break() outside of While()");
                if (loop_item->kind == LAI_LOOP_STACKITEM)
                    break;
                if (loop_item->kind != LAI_COND_STACKITEM && loop_item->kind != LAI_LOOP_STACKITEM)
                    lai_panic("Break() cannot skip item of type %d", loop_item->kind);
                m++;
            }

            // Pop all nested loops/conditions.
            for (int i = 0; i < m; i++) {
                lai_stackitem_t *pop_item = lai_exec_peek_stack_back(state);
                LAI_ENSURE(pop_item->kind == LAI_COND_STACKITEM
                           || pop_item->kind == LAI_LOOP_STACKITEM);
                lai_exec_pop_blkstack_back(state);
                lai_exec_pop_stack_back(state);
            }

            // Pop the LAI_LOOP_STACKITEM item.
            lai_exec_pop_blkstack_back(state);
            lai_exec_pop_stack_back(state);
            break;
        }
        /* If/Else Conditional */
        case IF_OP: {
            int if_pc = 0;
            int else_pc = 0;
            int has_else = 0;
            size_t if_size = 0;
            size_t else_size = 0;
            if (lai_parse_varint(&if_size, method, &pc, limit))
                return LAI_ERROR_EXECUTION_FAILURE;
            if_pc = pc;
            pc = opcode_pc + 1 + if_size;
            if (pc < block->limit && method[pc] == ELSE_OP) {
                has_else = 1;
                pc++;
                if (lai_parse_varint(&else_size, method, &pc, limit))
                    return LAI_ERROR_EXECUTION_FAILURE;
                else_pc = pc;
                pc = opcode_pc + 1 + if_size + 1 + else_size;
            }

            if (lai_exec_reserve_blkstack(state) || lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            struct lai_blkitem *blkitem = lai_exec_push_blkstack(state);
            blkitem->pc = if_pc;
            blkitem->limit = opcode_pc + 1 + if_size;

            lai_stackitem_t *cond_item = lai_exec_push_stack(state);
            cond_item->kind = LAI_COND_STACKITEM;
            cond_item->opstack_frame = state->opstack_ptr;
            cond_item->cond_state = 0;
            cond_item->cond_has_else = has_else;
            cond_item->cond_else_pc = else_pc;
            cond_item->cond_else_limit = opcode_pc + 1 + if_size + 1 + else_size;
            break;
        }
        case ELSE_OP:
            lai_panic("Else() outside of If()");
            break;

        // Scope-like objects in the ACPI namespace.
        case SCOPE_OP: {
            int nested_pc;
            size_t encoded_size;
            struct lai_amlname amln;
            if (lai_parse_varint(&encoded_size, method, &pc, limit)
                || lai_parse_name(&amln, method, &pc, limit))
                return LAI_ERROR_EXECUTION_FAILURE;
            nested_pc = pc;
            pc = opcode_pc + 1 + encoded_size;

            if (lai_exec_reserve_ctxstack(state) || lai_exec_reserve_blkstack(state)
                || lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_nsnode_t *scoped_ctx_handle = lai_do_resolve(ctx_handle, &amln);
            if (!scoped_ctx_handle) {
                lai_warn("Could not resolve node referenced in Scope");

                return LAI_ERROR_UNEXPECTED_RESULT;
            }

            struct lai_ctxitem *populate_ctxitem = lai_exec_push_ctxstack(state);
            populate_ctxitem->amls = amls;
            populate_ctxitem->code = method;
            populate_ctxitem->handle = scoped_ctx_handle;

            struct lai_blkitem *blkitem = lai_exec_push_blkstack(state);
            blkitem->pc = nested_pc;
            blkitem->limit = opcode_pc + 1 + encoded_size;

            lai_stackitem_t *item = lai_exec_push_stack(state);
            item->kind = LAI_POPULATE_STACKITEM;
            break;
        }
        case (EXTOP_PREFIX << 8) | DEVICE: {
            int nested_pc;
            size_t encoded_size;
            struct lai_amlname amln;
            if (lai_parse_varint(&encoded_size, method, &pc, limit)
                || lai_parse_name(&amln, method, &pc, limit))
                return LAI_ERROR_EXECUTION_FAILURE;
            nested_pc = pc;
            pc = opcode_pc + 2 + encoded_size;

            if (lai_exec_reserve_ctxstack(state) || lai_exec_reserve_blkstack(state)
                || lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_nsnode_t *node = lai_create_nsnode_or_die();
            node->type = LAI_NAMESPACE_DEVICE;
            lai_do_resolve_new_node(node, ctx_handle, &amln);
            LAI_TRY(lai_install_nsnode(node));

            if (invocation)
                lai_list_link(&invocation->per_method_list, &node->per_method_item);

            struct lai_ctxitem *populate_ctxitem = lai_exec_push_ctxstack(state);
            populate_ctxitem->amls = amls;
            populate_ctxitem->code = method;
            populate_ctxitem->handle = node;

            struct lai_blkitem *blkitem = lai_exec_push_blkstack(state);
            blkitem->pc = nested_pc;
            blkitem->limit = opcode_pc + 2 + encoded_size;

            lai_stackitem_t *item = lai_exec_push_stack(state);
            item->kind = LAI_POPULATE_STACKITEM;
            break;
        }
        case (EXTOP_PREFIX << 8) | PROCESSOR: {
            size_t pkgsize;
            struct lai_amlname amln;
            uint8_t cpu_id;
            uint32_t pblk_addr;
            uint8_t pblk_len;
            if (lai_parse_varint(&pkgsize, method, &pc, limit)
                || lai_parse_name(&amln, method, &pc, limit)
                || lai_parse_u8(&cpu_id, method, &pc, limit)
                || lai_parse_u32(&pblk_addr, method, &pc, limit)
                || lai_parse_u8(&pblk_len, method, &pc, limit))
                return LAI_ERROR_EXECUTION_FAILURE;
            int nested_pc = pc;
            pc = opcode_pc + 2 + pkgsize;

            if (lai_exec_reserve_ctxstack(state) || lai_exec_reserve_blkstack(state)
                || lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_nsnode_t *node = lai_create_nsnode_or_die();
            node->type = LAI_NAMESPACE_PROCESSOR;
            node->cpu_id = cpu_id;
            node->pblk_addr = pblk_addr;
            node->pblk_len = pblk_len;

            lai_do_resolve_new_node(node, ctx_handle, &amln);
            LAI_TRY(lai_install_nsnode(node));

            if (invocation)
                lai_list_link(&invocation->per_method_list, &node->per_method_item);

            struct lai_ctxitem *populate_ctxitem = lai_exec_push_ctxstack(state);
            populate_ctxitem->amls = amls;
            populate_ctxitem->code = method;
            populate_ctxitem->handle = node;

            struct lai_blkitem *blkitem = lai_exec_push_blkstack(state);
            blkitem->pc = nested_pc;
            blkitem->limit = opcode_pc + 2 + pkgsize;

            lai_stackitem_t *item = lai_exec_push_stack(state);
            item->kind = LAI_POPULATE_STACKITEM;
            break;
        }
        case (EXTOP_PREFIX << 8) | POWER_RES: {
            int nested_pc;
            size_t encoded_size;
            struct lai_amlname amln;
            if (lai_parse_varint(&encoded_size, method, &pc, limit)
                || lai_parse_name(&amln, method, &pc, limit))
                return LAI_ERROR_EXECUTION_FAILURE;
            //            uint8_t system_level = method[pc];
            pc++;
            //            uint16_t resource_order = *(uint16_t*)&method[pc];
            pc += 2;
            nested_pc = pc;
            pc = opcode_pc + 2 + encoded_size;

            if (lai_exec_reserve_ctxstack(state) || lai_exec_reserve_blkstack(state)
                || lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_nsnode_t *node = lai_create_nsnode_or_die();
            node->type = LAI_NAMESPACE_POWERRESOURCE;
            lai_do_resolve_new_node(node, ctx_handle, &amln);
            LAI_TRY(lai_install_nsnode(node));

            if (invocation)
                lai_list_link(&invocation->per_method_list, &node->per_method_item);

            struct lai_ctxitem *populate_ctxitem = lai_exec_push_ctxstack(state);
            populate_ctxitem->amls = amls;
            populate_ctxitem->code = method;
            populate_ctxitem->handle = node;

            struct lai_blkitem *blkitem = lai_exec_push_blkstack(state);
            blkitem->pc = nested_pc;
            blkitem->limit = opcode_pc + 2 + encoded_size;

            lai_stackitem_t *item = lai_exec_push_stack(state);
            item->kind = LAI_POPULATE_STACKITEM;
            break;
        }
        case (EXTOP_PREFIX << 8) | THERMALZONE: {
            int nested_pc;
            size_t encoded_size;
            struct lai_amlname amln;
            if (lai_parse_varint(&encoded_size, method, &pc, limit)
                || lai_parse_name(&amln, method, &pc, limit))
                return LAI_ERROR_EXECUTION_FAILURE;
            nested_pc = pc;
            pc = opcode_pc + 2 + encoded_size;

            if (lai_exec_reserve_ctxstack(state) || lai_exec_reserve_blkstack(state)
                || lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_nsnode_t *node = lai_create_nsnode_or_die();
            node->type = LAI_NAMESPACE_THERMALZONE;
            lai_do_resolve_new_node(node, ctx_handle, &amln);
            LAI_TRY(lai_install_nsnode(node));

            if (invocation)
                lai_list_link(&invocation->per_method_list, &node->per_method_item);

            struct lai_ctxitem *populate_ctxitem = lai_exec_push_ctxstack(state);
            populate_ctxitem->amls = amls;
            populate_ctxitem->code = method;
            populate_ctxitem->handle = node;

            struct lai_blkitem *blkitem = lai_exec_push_blkstack(state);
            blkitem->pc = nested_pc;
            blkitem->limit = opcode_pc + 2 + encoded_size;

            lai_stackitem_t *item = lai_exec_push_stack(state);
            item->kind = LAI_POPULATE_STACKITEM;
            break;
        }

        // Leafs in the ACPI namespace.
        case METHOD_OP: {
            size_t encoded_size;
            struct lai_amlname amln;
            uint8_t flags;
            if (lai_parse_varint(&encoded_size, method, &pc, limit)
                || lai_parse_name(&amln, method, &pc, limit)
                || lai_parse_u8(&flags, method, &pc, limit))
                return LAI_ERROR_EXECUTION_FAILURE;
            int nested_pc = pc;
            pc = opcode_pc + 1 + encoded_size;

            lai_exec_commit_pc(state, pc);

            lai_nsnode_t *node = lai_create_nsnode_or_die();
            node->type = LAI_NAMESPACE_METHOD;
            lai_do_resolve_new_node(node, ctx_handle, &amln);
            node->method_flags = flags;
            node->amls = amls;
            node->pointer = method + nested_pc;
            node->size = pc - nested_pc;
            LAI_TRY(lai_install_nsnode(node));

            if (invocation)
                lai_list_link(&invocation->per_method_list, &node->per_method_item);
            break;
        }
        case EXTERNAL_OP: {
            struct lai_amlname amln;
            uint8_t object_type;
            uint8_t argument_count;
            if (lai_parse_name(&amln, method, &pc, limit)
                || lai_parse_u8(&object_type, method, &pc, limit)
                || lai_parse_u8(&argument_count, method, &pc, limit))
                return LAI_ERROR_EXECUTION_FAILURE;

            lai_exec_commit_pc(state, pc);

            if (lai_current_instance()->trace & LAI_TRACE_OP) {
                LAI_CLEANUP_FREE_STRING char *path = lai_stringify_amlname(&amln);
                lai_debug(
                    "lai_exec_parse: ExternalOp, Name: %s, Object type: %02X, Argument Count: %01X",
                    path, object_type, argument_count);
            }
            break;
        }
        case NAME_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *node_item = lai_exec_push_stack(state);
            node_item->kind = LAI_NODE_STACKITEM;
            node_item->node_opcode = opcode;
            node_item->opstack_frame = state->opstack_ptr;
            node_item->node_arg_modes[0] = LAI_UNRESOLVED_MODE;
            node_item->node_arg_modes[1] = LAI_OBJECT_MODE;
            node_item->node_arg_modes[2] = 0;
            break;
        }
        case ALIAS_OP: {
            struct lai_amlname target_amln;
            struct lai_amlname dest_amln;
            if (lai_parse_name(&target_amln, method, &pc, limit)
                || lai_parse_name(&dest_amln, method, &pc, limit))
                return LAI_ERROR_EXECUTION_FAILURE;

            lai_exec_commit_pc(state, pc);

            lai_nsnode_t *node = lai_create_nsnode_or_die();
            node->type = LAI_NAMESPACE_ALIAS;
            node->al_target = lai_do_resolve(ctx_handle, &target_amln);
            if (!node->al_target)
                lai_panic("cannot resolve target %s of Alias()",
                          lai_stringify_amlname(&target_amln));
            lai_do_resolve_new_node(node, ctx_handle, &dest_amln);

            LAI_TRY(lai_install_nsnode(node));

            if (invocation)
                lai_list_link(&invocation->per_method_list, &node->per_method_item);
            break;
        }
        case BITFIELD_OP:
        case BYTEFIELD_OP:
        case WORDFIELD_OP:
        case DWORDFIELD_OP:
        case QWORDFIELD_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *node_item = lai_exec_push_stack(state);
            node_item->kind = LAI_NODE_STACKITEM;
            node_item->node_opcode = opcode;
            node_item->opstack_frame = state->opstack_ptr;
            node_item->node_arg_modes[0] = LAI_REFERENCE_MODE;
            node_item->node_arg_modes[1] = LAI_OBJECT_MODE;
            node_item->node_arg_modes[2] = LAI_UNRESOLVED_MODE;
            node_item->node_arg_modes[3] = 0;
            break;
        }
        case (EXTOP_PREFIX << 8) | ARBFIELD_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *node_item = lai_exec_push_stack(state);
            node_item->kind = LAI_NODE_STACKITEM;
            node_item->node_opcode = opcode;
            node_item->opstack_frame = state->opstack_ptr;
            node_item->node_arg_modes[0] = LAI_REFERENCE_MODE;
            node_item->node_arg_modes[1] = LAI_OBJECT_MODE;
            node_item->node_arg_modes[2] = LAI_OBJECT_MODE;
            node_item->node_arg_modes[3] = LAI_UNRESOLVED_MODE;
            node_item->node_arg_modes[4] = 0;
            break;
        }
        case (EXTOP_PREFIX << 8) | MUTEX: {
            struct lai_amlname amln;
            if (lai_parse_name(&amln, method, &pc, limit))
                return LAI_ERROR_EXECUTION_FAILURE;
            pc++; // skip over trailing 0x02

            lai_exec_commit_pc(state, pc);

            lai_nsnode_t *node = lai_create_nsnode_or_die();
            node->type = LAI_NAMESPACE_MUTEX;
            lai_do_resolve_new_node(node, ctx_handle, &amln);
            LAI_TRY(lai_install_nsnode(node));

            if (invocation)
                lai_list_link(&invocation->per_method_list, &node->per_method_item);
            break;
        }
        case (EXTOP_PREFIX << 8) | EVENT: {
            struct lai_amlname amln;
            if (lai_parse_name(&amln, method, &pc, limit))
                return LAI_ERROR_EXECUTION_FAILURE;

            lai_exec_commit_pc(state, pc);

            lai_nsnode_t *node = lai_create_nsnode_or_die();
            node->type = LAI_NAMESPACE_EVENT;
            lai_do_resolve_new_node(node, ctx_handle, &amln);
            LAI_TRY(lai_install_nsnode(node));

            if (invocation)
                lai_list_link(&invocation->per_method_list, &node->per_method_item);
            break;
        }
        case (EXTOP_PREFIX << 8) | OPREGION: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *node_item = lai_exec_push_stack(state);
            node_item->kind = LAI_NODE_STACKITEM;
            node_item->node_opcode = opcode;
            node_item->opstack_frame = state->opstack_ptr;
            node_item->node_arg_modes[0] = LAI_UNRESOLVED_MODE;
            node_item->node_arg_modes[1] = LAI_IMMEDIATE_BYTE_MODE;
            node_item->node_arg_modes[2] = LAI_OBJECT_MODE;
            node_item->node_arg_modes[3] = LAI_OBJECT_MODE;
            node_item->node_arg_modes[4] = 0;
            break;
        }
        case (EXTOP_PREFIX << 8) | FIELD: {
            size_t pkgsize;
            struct lai_amlname region_amln;
            if (lai_parse_varint(&pkgsize, method, &pc, limit)
                || lai_parse_name(&region_amln, method, &pc, limit))
                return LAI_ERROR_EXECUTION_FAILURE;

            int end_pc = opcode_pc + 2 + pkgsize;

            lai_nsnode_t *region_node = lai_do_resolve(ctx_handle, &region_amln);
            if (!region_node) {
                lai_panic("error parsing field for non-existant OpRegion, ignoring...");
                pc = end_pc;
                break;
            }

            uint8_t access_type = *(method + pc);
            pc++;

            // parse FieldList
            struct lai_amlname field_amln;
            uint64_t curr_off = 0;
            size_t skip_bits;
            while (pc < end_pc) {
                switch (*(method + pc)) {
                    case 0: // ReservedField
                        pc++;
                        // TODO: Partially failing to parse a Field() is a bad idea.
                        if (lai_parse_varint(&skip_bits, method, &pc, limit))
                            return LAI_ERROR_EXECUTION_FAILURE;
                        curr_off += skip_bits;
                        break;
                    case 1: // AccessField
                        pc++;
                        access_type = *(method + pc);
                        pc += 2;
                        break;
                    case 2: // TODO: ConnectField
                        lai_panic("ConnectField parsing isn't implemented");
                        break;
                    default: // NamedField
                             // TODO: Partially failing to parse a Field() is a bad idea.
                        if (lai_parse_name(&field_amln, method, &pc, limit)
                            || lai_parse_varint(&skip_bits, method, &pc, limit))
                            return LAI_ERROR_EXECUTION_FAILURE;

                        lai_nsnode_t *node = lai_create_nsnode_or_die();
                        node->type = LAI_NAMESPACE_FIELD;
                        node->fld_region_node = region_node;
                        node->fld_flags = access_type;
                        node->fld_size = skip_bits;
                        node->fld_offset = curr_off;
                        lai_do_resolve_new_node(node, ctx_handle, &field_amln);
                        LAI_TRY(lai_install_nsnode(node));

                        if (invocation)
                            lai_list_link(&invocation->per_method_list, &node->per_method_item);

                        curr_off += skip_bits;
                }
            }
            lai_exec_commit_pc(state, pc);

            break;
        }
        case (EXTOP_PREFIX << 8) | INDEXFIELD: {
            size_t pkgsize;
            struct lai_amlname index_amln;
            struct lai_amlname data_amln;
            if (lai_parse_varint(&pkgsize, method, &pc, limit)
                || lai_parse_name(&index_amln, method, &pc, limit)
                || lai_parse_name(&data_amln, method, &pc, limit))
                return LAI_ERROR_EXECUTION_FAILURE;

            int end_pc = opcode_pc + 2 + pkgsize;

            lai_nsnode_t *index_node = lai_do_resolve(ctx_handle, &index_amln);
            lai_nsnode_t *data_node = lai_do_resolve(ctx_handle, &data_amln);
            if (!index_node || !data_node)
                lai_panic("could not resolve index register of IndexField()");

            uint8_t access_type = *(method + pc);
            pc++;

            // parse FieldList
            struct lai_amlname field_amln;
            uint64_t curr_off = 0;
            size_t skip_bits;
            while (pc < end_pc) {
                switch (*(method + pc)) {
                    case 0: // ReservedField
                        pc++;
                        // TODO: Partially failing to parse a Field() is a bad idea.
                        if (lai_parse_varint(&skip_bits, method, &pc, limit))
                            return LAI_ERROR_EXECUTION_FAILURE;
                        curr_off += skip_bits;
                        break;
                    case 1: // AccessField
                        pc++;
                        access_type = *(method + pc);
                        pc += 2;
                        break;
                    case 2: // TODO: ConnectField
                        lai_panic("ConnectField parsing isn't implemented");
                        break;
                    default: // NamedField
                        // TODO: Partially failing to parse a Field() is a bad idea.
                        if (lai_parse_name(&field_amln, method, &pc, limit)
                            || lai_parse_varint(&skip_bits, method, &pc, limit))
                            return LAI_ERROR_EXECUTION_FAILURE;

                        lai_nsnode_t *node = lai_create_nsnode_or_die();
                        node->type = LAI_NAMESPACE_INDEXFIELD;
                        node->fld_idxf_index_node = index_node;
                        node->fld_idxf_data_node = data_node;
                        node->fld_flags = access_type;
                        node->fld_size = skip_bits;
                        node->fld_offset = curr_off;
                        lai_do_resolve_new_node(node, ctx_handle, &field_amln);
                        LAI_TRY(lai_install_nsnode(node));

                        if (invocation)
                            lai_list_link(&invocation->per_method_list, &node->per_method_item);

                        curr_off += skip_bits;
                }
            }
            lai_exec_commit_pc(state, pc);

            break;
        }

        case (EXTOP_PREFIX << 8) | BANKFIELD: {
            size_t pkgsize;
            struct lai_amlname region_amln;
            struct lai_amlname bank_amln;
            if (lai_parse_varint(&pkgsize, method, &pc, limit)
                || lai_parse_name(&region_amln, method, &pc, limit)
                || lai_parse_name(&bank_amln, method, &pc, limit))
                return LAI_ERROR_EXECUTION_FAILURE;

            int start_pc = pc;
            pc = opcode_pc + 2 + pkgsize;

            lai_nsnode_t *region_node = lai_do_resolve(ctx_handle, &region_amln);
            lai_nsnode_t *bank_node = lai_do_resolve(ctx_handle, &bank_amln);
            if (!region_node || !bank_node)
                lai_panic("could not resolve region/bank of BankField()");

            if (lai_exec_reserve_blkstack(state) || lai_exec_reserve_stack(state)
                || lai_exec_reserve_opstack_n(state, 2))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            struct lai_blkitem *blkitem = lai_exec_push_blkstack(state);
            blkitem->pc = start_pc;
            blkitem->limit = pc;

            lai_stackitem_t *bankfield_item = lai_exec_push_stack(state);
            bankfield_item->kind = LAI_BANKFIELD_STACKITEM;
            bankfield_item->opstack_frame = state->opstack_ptr;

            struct lai_operand *region_operand = lai_exec_push_opstack(state);
            region_operand->tag = LAI_RESOLVED_NAME;
            region_operand->handle = region_node;

            struct lai_operand *bank_operand = lai_exec_push_opstack(state);
            bank_operand->tag = LAI_RESOLVED_NAME;
            bank_operand->handle = bank_node;

            break;
        }

        case ARG0_OP:
        case ARG1_OP:
        case ARG2_OP:
        case ARG3_OP:
        case ARG4_OP:
        case ARG5_OP:
        case ARG6_OP: {
            if (lai_exec_reserve_opstack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            if (parse_mode == LAI_REFERENCE_MODE || parse_mode == LAI_OPTIONAL_REFERENCE_MODE) {
                struct lai_operand *opstack_res = lai_exec_push_opstack(state);
                opstack_res->tag = LAI_ARG_NAME;
                opstack_res->index = opcode - ARG0_OP;
            } else {
                LAI_ENSURE(parse_mode == LAI_OBJECT_MODE);
                struct lai_operand *opstack_res = lai_exec_push_opstack(state);
                opstack_res->tag = LAI_OPERAND_OBJECT;
                LAI_ENSURE(invocation);
                lai_var_assign(&opstack_res->object, &invocation->arg[opcode - ARG0_OP]);
            }
            break;
        }

        case LOCAL0_OP:
        case LOCAL1_OP:
        case LOCAL2_OP:
        case LOCAL3_OP:
        case LOCAL4_OP:
        case LOCAL5_OP:
        case LOCAL6_OP:
        case LOCAL7_OP: {
            if (lai_exec_reserve_opstack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            if (parse_mode == LAI_REFERENCE_MODE || parse_mode == LAI_OPTIONAL_REFERENCE_MODE) {
                struct lai_operand *opstack_res = lai_exec_push_opstack(state);
                opstack_res->tag = LAI_LOCAL_NAME;
                opstack_res->index = opcode - LOCAL0_OP;
            } else {
                LAI_ENSURE(parse_mode == LAI_OBJECT_MODE);
                struct lai_operand *opstack_res = lai_exec_push_opstack(state);
                opstack_res->tag = LAI_OPERAND_OBJECT;
                LAI_ENSURE(invocation);
                lai_var_assign(&opstack_res->object, &invocation->local[opcode - LOCAL0_OP]);
            }
            break;
        }

        case BREAKPOINT_OP: {
            lai_exec_commit_pc(state, pc);
            lai_debug("Encountered BreakPointOp");
            break;
        }

        case TOBUFFER_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;

            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[1] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[2] = 0;
            op_item->op_want_result = want_result;
            break;
        }

        case TODECIMALSTRING_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;

            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[1] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[2] = 0;
            op_item->op_want_result = want_result;
            break;
        }

        case TOHEXSTRING_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;

            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[1] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[2] = 0;
            op_item->op_want_result = want_result;
            break;
        }

        case TOINTEGER_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;

            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[1] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[2] = 0;
            op_item->op_want_result = want_result;
            break;
        }

        case TOSTRING_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;

            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[1] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[2] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[3] = 0;
            op_item->op_want_result = want_result;
            break;
        }

        case MID_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;

            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[1] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[2] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[3] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[4] = 0;
            op_item->op_want_result = want_result;
            break;
        }

        case (EXTOP_PREFIX << 8) | FATAL_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;

            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_IMMEDIATE_BYTE_MODE;
            op_item->op_arg_modes[1] = LAI_IMMEDIATE_DWORD_MODE;
            op_item->op_arg_modes[2] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[3] = 0;
            op_item->op_want_result = want_result;
            break;
        }

        case (EXTOP_PREFIX << 8) | DEBUG_OP: {
            if (lai_exec_reserve_opstack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            // Accessing (i.e., loading from) the Debug object is not supported yet.
            LAI_ENSURE(parse_mode == LAI_REFERENCE_MODE
                       || parse_mode == LAI_OPTIONAL_REFERENCE_MODE);
            struct lai_operand *result = lai_exec_push_opstack(state);
            result->tag = LAI_DEBUG_NAME;
            break;
        }

        case STORE_OP:
        case COPYOBJECT_OP:
        case NOT_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[1] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[2] = 0;
            op_item->op_want_result = want_result;
            break;
        }
        case FINDSETLEFTBIT_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[1] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[2] = 0;
            op_item->op_want_result = want_result;
            break;
        }
        case FINDSETRIGHTBIT_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[1] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[2] = 0;
            op_item->op_want_result = want_result;
            break;
        }
        case CONCAT_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[1] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[2] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[3] = 0;
            op_item->op_want_result = want_result;
            break;
        }
        case ADD_OP:
        case SUBTRACT_OP:
        case MOD_OP:
        case MULTIPLY_OP:
        case AND_OP:
        case OR_OP:
        case XOR_OP:
        case SHR_OP:
        case SHL_OP:
        case NAND_OP:
        case NOR_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[1] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[2] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[3] = 0;
            op_item->op_want_result = want_result;
            break;
        }
        case DIVIDE_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[1] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[2] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[3] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[4] = 0;
            op_item->op_want_result = want_result;
            break;
        }

        case INCREMENT_OP:
        case DECREMENT_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[1] = 0;
            op_item->op_want_result = want_result;
            break;
        }

        case LNOT_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[1] = 0;
            op_item->op_want_result = want_result;
            break;
        }
        case LAND_OP:
        case LOR_OP:
        case LEQUAL_OP:
        case LLESS_OP:
        case LGREATER_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[1] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[2] = 0;
            op_item->op_want_result = want_result;
            break;
        }

        case INDEX_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[1] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[2] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[3] = 0;
            op_item->op_want_result = want_result;
            break;
        }
        case MATCH_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[1] = LAI_IMMEDIATE_BYTE_MODE;
            op_item->op_arg_modes[2] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[3] = LAI_IMMEDIATE_BYTE_MODE;
            op_item->op_arg_modes[4] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[5] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[6] = 0;
            op_item->op_want_result = want_result;
            break;
        }
        case CONCATRES_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[1] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[2] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[3] = 0;
            op_item->op_want_result = want_result;
            break;
        }
        case OBJECTTYPE_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[1] = 0;
            op_item->op_want_result = want_result;
            break;
        }
        case DEREF_OP:
        case SIZEOF_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[1] = 0;
            op_item->op_want_result = want_result;
            break;
        }
        case REFOF_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[1] = 0;
            op_item->op_want_result = want_result;
            break;
        }

        case NOTIFY_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[1] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[2] = 0;
            op_item->op_want_result = want_result;
            break;
        }

        case (EXTOP_PREFIX << 8) | CONDREF_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_OPTIONAL_REFERENCE_MODE;
            op_item->op_arg_modes[1] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[2] = 0;
            op_item->op_want_result = want_result;
            break;
        }

        case (EXTOP_PREFIX << 8) | STALL_OP:
        case (EXTOP_PREFIX << 8) | SLEEP_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[1] = 0;
            op_item->op_want_result = want_result;
            break;
        }

        case (EXTOP_PREFIX << 8) | ACQUIRE_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[1] = LAI_IMMEDIATE_WORD_MODE;
            op_item->op_arg_modes[2] = 0;
            op_item->op_want_result = want_result;
            break;
        }
        case (EXTOP_PREFIX << 8) | RELEASE_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[1] = 0;
            op_item->op_want_result = want_result;
            break;
        }

        case (EXTOP_PREFIX << 8) | WAIT_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[1] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[2] = 0;
            op_item->op_want_result = want_result;
            break;
        }
        case (EXTOP_PREFIX << 8) | SIGNAL_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[1] = 0;
            op_item->op_want_result = want_result;
            break;
        }
        case (EXTOP_PREFIX << 8) | RESET_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[1] = 0;
            op_item->op_want_result = want_result;
            break;
        }

        case (EXTOP_PREFIX << 8) | FROM_BCD_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[1] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[2] = 0;
            op_item->op_want_result = want_result;
            break;
        }
        case (EXTOP_PREFIX << 8) | TO_BCD_OP: {
            if (lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;
            lai_exec_commit_pc(state, pc);

            lai_stackitem_t *op_item = lai_exec_push_stack(state);
            op_item->kind = LAI_OP_STACKITEM;
            op_item->op_opcode = opcode;
            op_item->opstack_frame = state->opstack_ptr;
            op_item->op_arg_modes[0] = LAI_OBJECT_MODE;
            op_item->op_arg_modes[1] = LAI_REFERENCE_MODE;
            op_item->op_arg_modes[2] = 0;
            op_item->op_want_result = want_result;
            break;
        }

        default:
            lai_panic("unexpected opcode in lai_exec_run(), sequence %02X %02X %02X %02X",
                      method[opcode_pc + 0], method[opcode_pc + 1], method[opcode_pc + 2],
                      method[opcode_pc + 3]);
    }
    return LAI_ERROR_NONE;
}

lai_api_error_t lai_populate(lai_nsnode_t *parent, struct lai_aml_segment *amls,
                             lai_state_t *state) {
    if (lai_exec_reserve_ctxstack(state) || lai_exec_reserve_blkstack(state)
        || lai_exec_reserve_stack(state))
        return LAI_ERROR_OUT_OF_MEMORY;

    size_t size = amls->table->header.length - sizeof(acpi_header_t);

    struct lai_ctxitem *populate_ctxitem = lai_exec_push_ctxstack(state);
    populate_ctxitem->amls = amls;
    populate_ctxitem->code = amls->table->data;
    populate_ctxitem->handle = parent;

    struct lai_blkitem *blkitem = lai_exec_push_blkstack(state);
    blkitem->pc = 0;
    blkitem->limit = size;

    lai_stackitem_t *item = lai_exec_push_stack(state);
    item->kind = LAI_POPULATE_STACKITEM;

    int status = lai_exec_run(state);
    if (status != LAI_ERROR_NONE) {
        lai_warn("lai_exec_run() failed in lai_populate()");
        return status;
    }
    LAI_ENSURE(state->ctxstack_ptr == -1);
    LAI_ENSURE(state->stack_ptr == -1);
    LAI_ENSURE(!state->opstack_ptr);
    return LAI_ERROR_NONE;
}

// lai_eval_args(): Evaluates a node of the ACPI namespace (including control methods).
lai_api_error_t lai_eval_args(lai_variable_t *result, lai_nsnode_t *handle, lai_state_t *state,
                              int n, lai_variable_t *args) {
    LAI_ENSURE(handle);
    LAI_ENSURE(handle->type != LAI_NAMESPACE_ALIAS);

    switch (handle->type) {
        case LAI_NAMESPACE_NAME:
            if (n) {
                lai_warn("non-empty argument list given when evaluating Name()");
                return LAI_ERROR_TYPE_MISMATCH;
            }
            if (result)
                lai_obj_clone(result, &handle->object);
            return LAI_ERROR_NONE;
        case LAI_NAMESPACE_METHOD: {
            if (lai_exec_reserve_ctxstack(state) || lai_exec_reserve_blkstack(state)
                || lai_exec_reserve_stack(state))
                return LAI_ERROR_OUT_OF_MEMORY;

            LAI_CLEANUP_VAR lai_variable_t method_result = LAI_VAR_INITIALIZER;
            int e;
            if (handle->method_override) {
                // It's an OS-defined method.
                // TODO: Verify the number of argument to the overridden method.
                e = handle->method_override(args, &method_result);
            } else {
                // It's an AML method.
                LAI_ENSURE(handle->amls);

                struct lai_ctxitem *method_ctxitem = lai_exec_push_ctxstack(state);
                method_ctxitem->amls = handle->amls;
                method_ctxitem->code = handle->pointer;
                method_ctxitem->handle = handle;
                method_ctxitem->invocation = laihost_malloc(sizeof(struct lai_invocation));
                if (!method_ctxitem->invocation)
                    lai_panic("could not allocate memory for method invocation");
                memset(method_ctxitem->invocation, 0, sizeof(struct lai_invocation));
                lai_list_init(&method_ctxitem->invocation->per_method_list);

                for (int i = 0; i < n; i++)
                    lai_var_assign(&method_ctxitem->invocation->arg[i], &args[i]);

                struct lai_blkitem *blkitem = lai_exec_push_blkstack(state);
                blkitem->pc = 0;
                blkitem->limit = handle->size;

                lai_stackitem_t *item = lai_exec_push_stack(state);
                item->kind = LAI_METHOD_STACKITEM;
                item->mth_want_result = 1;

                e = lai_exec_run(state);

                if (e == LAI_ERROR_NONE) {
                    LAI_ENSURE(state->ctxstack_ptr == -1);
                    LAI_ENSURE(state->stack_ptr == -1);
                    if (state->opstack_ptr != 1) // This would be an internal error.
                        lai_panic("expected exactly one return value after method invocation");
                    struct lai_operand *opstack_top = lai_exec_get_opstack(state, 0);
                    lai_variable_t objectref = {0};
                    lai_exec_get_objectref(state, opstack_top, &objectref);
                    lai_obj_clone(&method_result, &objectref);
                    lai_var_finalize(&objectref);
                    lai_exec_pop_opstack(state, 1);
                } else {
                    // If there is an error the lai_state_t is probably corrupted, we should reset
                    // it
                    lai_finalize_state(state);
                    lai_init_state(state);
                }
            }
            if (e == LAI_ERROR_NONE && result)
                lai_var_move(result, &method_result);
            return e;
        }

        default:
            return LAI_ERROR_TYPE_MISMATCH;
    }
}

lai_api_error_t lai_eval_vargs(lai_variable_t *result, lai_nsnode_t *handle, lai_state_t *state,
                               va_list vl) {
    int n = 0;
    lai_variable_t args[7];
    memset(args, 0, sizeof(lai_variable_t) * 7);

    for (;;) {
        LAI_ENSURE(n < 7 && "ACPI supports at most 7 arguments");
        lai_variable_t *object = va_arg(vl, lai_variable_t *);
        if (!object)
            break;
        lai_var_assign(&args[n++], object);
    }

    return lai_eval_args(result, handle, state, n, args);
}

lai_api_error_t lai_eval_largs(lai_variable_t *result, lai_nsnode_t *handle, lai_state_t *state,
                               ...) {
    va_list vl;
    va_start(vl, state);
    int e = lai_eval_vargs(result, handle, state, vl);
    va_end(vl);
    return e;
}

lai_api_error_t lai_eval(lai_variable_t *result, lai_nsnode_t *handle, lai_state_t *state) {
    return lai_eval_args(result, handle, state, 0, NULL);
}

void lai_enable_tracing(int trace) {
    lai_current_instance()->trace = trace;
}

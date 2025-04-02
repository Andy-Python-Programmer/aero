/*
 * Lightweight AML Interpreter
 * Copyright (C) 2018-2023 The lai authors
 */

#include <lai/core.h>

#include "aml_opcodes.h"
#include "exec_impl.h"
#include "libc.h"

lai_api_error_t lai_create_string(lai_variable_t *object, size_t length) {
    object->type = LAI_STRING;
    object->string_ptr = laihost_malloc(sizeof(struct lai_string_head));
    if (!object->string_ptr)
        return LAI_ERROR_OUT_OF_MEMORY;
    object->string_ptr->rc = 1;
    object->string_ptr->content = laihost_malloc(length + 1);
    object->string_ptr->capacity = length + 1;
    if (!object->string_ptr->content) {
        laihost_free(object->string_ptr, sizeof(struct lai_string_head));
        return LAI_ERROR_OUT_OF_MEMORY;
    }
    memset(object->string_ptr->content, 0, length + 1);
    return LAI_ERROR_NONE;
}

lai_api_error_t lai_create_c_string(lai_variable_t *object, const char *s) {
    size_t n = lai_strlen(s);
    lai_api_error_t e = lai_create_string(object, n);
    if (e != LAI_ERROR_NONE)
        return e;
    memcpy(lai_exec_string_access(object), s, n);
    return LAI_ERROR_NONE;
}

lai_api_error_t lai_create_buffer(lai_variable_t *object, size_t size) {
    object->type = LAI_BUFFER;
    object->buffer_ptr = laihost_malloc(sizeof(struct lai_buffer_head));
    if (!object->buffer_ptr)
        return LAI_ERROR_OUT_OF_MEMORY;
    object->buffer_ptr->rc = 1;
    object->buffer_ptr->size = size;
    object->buffer_ptr->content = laihost_malloc(size);
    if (!object->buffer_ptr->content) {
        laihost_free(object->buffer_ptr, sizeof(struct lai_buffer_head));
        return LAI_ERROR_OUT_OF_MEMORY;
    }
    memset(object->buffer_ptr->content, 0, size);
    return LAI_ERROR_NONE;
}

lai_api_error_t lai_create_pkg(lai_variable_t *object, size_t n) {
    object->type = LAI_PACKAGE;
    object->pkg_ptr = laihost_malloc(sizeof(struct lai_pkg_head));
    if (!object->pkg_ptr)
        return LAI_ERROR_OUT_OF_MEMORY;
    object->pkg_ptr->rc = 1;
    object->pkg_ptr->size = n;
    object->pkg_ptr->elems = laihost_malloc(n * sizeof(lai_variable_t));
    if (!object->pkg_ptr->elems) {
        laihost_free(object->pkg_ptr, sizeof(struct lai_pkg_head));
        return LAI_ERROR_OUT_OF_MEMORY;
    }
    memset(object->pkg_ptr->elems, 0, n * sizeof(lai_variable_t));
    return LAI_ERROR_NONE;
}

lai_api_error_t lai_obj_resize_string(lai_variable_t *object, size_t length) {
    if (object->type != LAI_STRING)
        return LAI_ERROR_TYPE_MISMATCH;
    if (length > lai_strlen(object->string_ptr->content)) {
        char *new_content = laihost_malloc(length + 1);
        if (!new_content)
            return LAI_ERROR_OUT_OF_MEMORY;
        lai_strcpy(new_content, object->string_ptr->content);
        laihost_free(object->string_ptr->content, object->string_ptr->capacity);
        object->string_ptr->content = new_content;
        object->string_ptr->capacity = length + 1;
    }
    return LAI_ERROR_NONE;
}

lai_api_error_t lai_obj_resize_buffer(lai_variable_t *object, size_t size) {
    if (object->type != LAI_BUFFER)
        return LAI_ERROR_TYPE_MISMATCH;
    if (size > object->buffer_ptr->size) {
        uint8_t *new_content = laihost_malloc(size);
        if (!new_content)
            return LAI_ERROR_OUT_OF_MEMORY;
        memset(new_content, 0, size);
        memcpy(new_content, object->buffer_ptr->content, object->buffer_ptr->size);
        laihost_free(object->buffer_ptr->content, object->buffer_ptr->size);
        object->buffer_ptr->content = new_content;
    }
    object->buffer_ptr->size = size;
    return LAI_ERROR_NONE;
}

lai_api_error_t lai_obj_resize_pkg(lai_variable_t *object, size_t n) {
    if (object->type != LAI_PACKAGE)
        return LAI_ERROR_TYPE_MISMATCH;
    if (n <= object->pkg_ptr->size) {
        for (unsigned int i = n; i < object->pkg_ptr->size; i++)
            lai_var_finalize(&object->pkg_ptr->elems[i]);
    } else {
        struct lai_variable_t *new_elems = laihost_malloc(n * sizeof(lai_variable_t));
        if (!new_elems)
            return LAI_ERROR_OUT_OF_MEMORY;
        memset(new_elems, 0, n * sizeof(lai_variable_t));
        for (unsigned int i = 0; i < object->pkg_ptr->size; i++)
            lai_var_move(&new_elems[i], &object->pkg_ptr->elems[i]);
        laihost_free(object->pkg_ptr->elems, object->pkg_ptr->size * sizeof(lai_variable_t));
        object->pkg_ptr->elems = new_elems;
    }
    object->pkg_ptr->size = n;
    return LAI_ERROR_NONE;
}

static enum lai_object_type lai_object_type_of_objref(lai_variable_t *object) {
    switch (object->type) {
        case LAI_INTEGER:
            return LAI_TYPE_INTEGER;
        case LAI_STRING:
            return LAI_TYPE_STRING;
        case LAI_BUFFER:
            return LAI_TYPE_BUFFER;
        case LAI_PACKAGE:
            return LAI_TYPE_PACKAGE;

        default:
            lai_panic("unexpected object type %d in lai_object_type_of_objref()", object->type);
    }
}

static enum lai_object_type lai_object_type_of_node(lai_nsnode_t *handle) {
    switch (handle->type) {
        case LAI_NAMESPACE_DEVICE:
            return LAI_TYPE_DEVICE;

        default:
            lai_panic("unexpected node type %d in lai_object_type_of_node()", handle->type);
    }
}

enum lai_object_type lai_obj_get_type(lai_variable_t *object) {
    switch (object->type) {
        case LAI_INTEGER:
        case LAI_STRING:
        case LAI_BUFFER:
        case LAI_PACKAGE:
            return lai_object_type_of_objref(object);

        case LAI_HANDLE:
            return lai_object_type_of_node(object->handle);
        case LAI_LAZY_HANDLE: {
            struct lai_amlname amln;
            lai_amlname_parse(&amln, object->unres_aml);

            lai_nsnode_t *handle = lai_do_resolve(object->unres_ctx_handle, &amln);
            if (!handle)
                lai_panic("undefined reference %s", lai_stringify_amlname(&amln));
            return lai_object_type_of_node(handle);
        }
        case 0:
            return LAI_TYPE_NONE;
        default:
            lai_panic("unexpected object type %d for lai_obj_get_type()", object->type);
    }
}

lai_api_error_t lai_obj_get_integer(lai_variable_t *object, uint64_t *out) {
    switch (object->type) {
        case LAI_INTEGER:
            *out = object->integer;
            return LAI_ERROR_NONE;

        default:
            lai_warn("lai_obj_get_integer() expects an integer, not a value of type %d",
                     object->type);
            return LAI_ERROR_TYPE_MISMATCH;
    }
}

lai_api_error_t lai_obj_get_pkg(lai_variable_t *object, size_t i, lai_variable_t *out) {
    if (object->type != LAI_PACKAGE)
        return LAI_ERROR_TYPE_MISMATCH;
    if (i >= lai_exec_pkg_size(object))
        return LAI_ERROR_OUT_OF_BOUNDS;
    lai_exec_pkg_load(out, object, i);
    return 0;
}

lai_api_error_t lai_obj_get_handle(lai_variable_t *object, lai_nsnode_t **out) {
    switch (object->type) {
        case LAI_HANDLE:
            *out = object->handle;
            return LAI_ERROR_NONE;
        case LAI_LAZY_HANDLE: {
            struct lai_amlname amln;
            lai_amlname_parse(&amln, object->unres_aml);

            lai_nsnode_t *handle = lai_do_resolve(object->unres_ctx_handle, &amln);
            if (!handle)
                lai_panic("undefined reference %s", lai_stringify_amlname(&amln));
            *out = handle;
            return LAI_ERROR_NONE;
        }

        default:
            lai_warn("lai_obj_get_handle() expects a handle type, not a value of type %d",
                     object->type);
            return LAI_ERROR_TYPE_MISMATCH;
    }
}

lai_api_error_t lai_obj_to_buffer(lai_variable_t *out, lai_variable_t *object) {
    switch (object->type) {
        case LAI_TYPE_INTEGER:
            if (lai_create_buffer(out, sizeof(uint64_t)) != LAI_ERROR_NONE)
                return LAI_ERROR_OUT_OF_MEMORY;
            memcpy(out->buffer_ptr->content, &object->integer, sizeof(uint64_t));
            break;

        case LAI_TYPE_BUFFER:
            lai_obj_clone(out, object);
            break;

        case LAI_TYPE_STRING: {
            size_t len = lai_exec_string_length(object);
            if (len == 0) {
                if (lai_create_buffer(out, 0) != LAI_ERROR_NONE)
                    return LAI_ERROR_OUT_OF_MEMORY;
            } else {
                if (lai_create_buffer(out, len + 1) != LAI_ERROR_NONE)
                    return LAI_ERROR_OUT_OF_MEMORY;
                memcpy(out->buffer_ptr->content, object->string_ptr->content, len);
            }
            break;
        }

        default:
            lai_warn("lai_obj_to_buffer() unsupported object type %d", object->type);
            return LAI_ERROR_ILLEGAL_ARGUMENTS;
    }

    return LAI_ERROR_NONE;
}

lai_api_error_t lai_mutate_buffer(lai_variable_t *target, lai_variable_t *object) {
    // Buffers are *not* resized during mutation.
    // The target buffer determines the size of the result.

    switch (object->type) {
        // No conversion necessary.
        case LAI_BUFFER: {
            size_t copy_size = lai_exec_buffer_size(object);
            size_t buffer_size = lai_exec_buffer_size(target);
            if (copy_size > buffer_size)
                copy_size = buffer_size;
            memset(lai_exec_buffer_access(target), 0, buffer_size);
            memcpy(lai_exec_buffer_access(target), lai_exec_buffer_access(object), copy_size);
            break;
        }

        case LAI_INTEGER: {
            // TODO: This assumes that 64-bit integers are used.
            size_t copy_size = 8;
            size_t buffer_size = lai_exec_buffer_size(target);
            if (copy_size > buffer_size)
                copy_size = buffer_size;
            // TODO: bswap() if necessary.
            uint64_t data = object->integer;
            memset(lai_exec_buffer_access(target), 0, buffer_size);
            memcpy(lai_exec_buffer_access(target), &data, copy_size);
            break;
        }
        case LAI_STRING: {
            size_t copy_size = lai_strlen(lai_exec_string_access(object)) + 1;
            size_t buffer_size = lai_exec_buffer_size(target);
            if (copy_size > buffer_size)
                copy_size = buffer_size;
            memset(lai_exec_buffer_access(target), 0, buffer_size);
            memcpy(lai_exec_buffer_access(target), lai_exec_string_access(object), copy_size);
            break;
        }

        default:
            lai_warn("lai_mutate_buffer() unsupported object type %d", object->type);
            return LAI_ERROR_ILLEGAL_ARGUMENTS;
    }

    return LAI_ERROR_NONE;
}

lai_api_error_t lai_obj_to_string(lai_variable_t *out, lai_variable_t *object, size_t size) {
    switch (object->type) {
        case LAI_TYPE_BUFFER: {
            size_t buffer_length = 0;
            uint8_t *buffer = lai_exec_buffer_access(object);
            for (uint64_t i = 0; i < lai_exec_buffer_size(object); i++) {
                if (buffer[i] == '\0')
                    break;
                buffer_length++;
            }

            if (buffer_length == 0) {
                lai_create_string(out, 0);
            } else if (size == ~(size_t)(0)) {
                // Copy until the '\0'
                lai_create_string(out, buffer_length + 1);
                char *string = lai_exec_string_access(out);
                memcpy(string, buffer, buffer_length);
            } else {
                if (size < buffer_length) {
                    lai_create_string(out, size);
                    char *string = lai_exec_string_access(out);
                    memcpy(string, buffer, size);
                } else {
                    lai_create_string(out, buffer_length);
                    char *string = lai_exec_string_access(out);
                    memcpy(string, buffer, buffer_length);
                }
            }
            break;
        }

        default:
            lai_warn("lai_obj_to_string() unsupported object type %d", object->type);
            return LAI_ERROR_ILLEGAL_ARGUMENTS;
    }

    return LAI_ERROR_NONE;
}

lai_api_error_t lai_obj_to_decimal_string(lai_variable_t *out, lai_variable_t *object) {
    switch (object->type) {
        case LAI_INTEGER: {
            lai_create_string(out, 20); // Max length for 64-bit integer is 20 chars
            char *s = lai_exec_string_access(out);
            lai_snprintf(s, 21, "%llu", object->integer); // snprintf null terminates
            break;
        }

        case LAI_BUFFER: {
            size_t buffer_len = lai_exec_buffer_size(object);
            uint8_t *buffer = lai_exec_buffer_access(object);
            lai_create_string(
                out,
                (buffer_len * 3)); // For every buffer byte we need 2 chars of number and a comma

            char *string = lai_exec_string_access(out);
            uint64_t string_index = 0;

            for (uint64_t i = 0; i < buffer_len; i++) {
                char buf[5] = "";
                lai_snprintf(buf, 5, "%02d", buffer[i]);

                string[string_index] = buf[0];
                string[string_index + 1] = buf[1];
                string[string_index + 2] = ',';
                string_index += 3;
            }
            // String with values should be constructed now, remove the last comma
            string[string_index - 1] = '\0';
            break;
        }

        case LAI_STRING:
            lai_obj_clone(out, object);
            break;

        default:
            lai_warn("lai_obj_to_decimal_string() unsupported object type %d", object->type);
            return LAI_ERROR_ILLEGAL_ARGUMENTS;
    }

    return LAI_ERROR_NONE;
}

// The spec doesn't mention this but the numbers should be prefixed with 0x
lai_api_error_t lai_obj_to_hex_string(lai_variable_t *out, lai_variable_t *object) {
    switch (object->type) {
        case LAI_INTEGER: {
            lai_create_string(
                out, 16); // 64-bit integer is 8 bytes, each byte takes 2 chars, is 16 chars
            char *s = lai_exec_string_access(out);
            lai_snprintf(s, 17, "%X", object->integer); // snprintf null terminates
            break;
        }

        case LAI_BUFFER: {
            size_t buffer_len = lai_exec_buffer_size(object);
            uint8_t *buffer = lai_exec_buffer_access(object);
            lai_create_string(
                out,
                (buffer_len
                 * 5)); // For every buffer byte we need 2 chars of prefix, 2 chars of number and a
                        // comma I'll take the 1 byte loss of the last comma for code simplicity

            char *string = lai_exec_string_access(out);
            uint64_t string_index = 0;

            for (uint64_t i = 0; i < buffer_len; i++) {
                char buf[5] = "";
                lai_snprintf(buf, 5, "%02X", buffer[i]);

                string[string_index] = '0';
                string[string_index + 1] = 'x';
                string[string_index + 2] = buf[0];
                string[string_index + 3] = buf[1];
                string[string_index + 4] = ',';
                string_index += 5;
            }
            // String with values should be constructed now, remove the last comma
            string[string_index - 1] = '\0';
            break;
        }

        case LAI_STRING:
            lai_obj_clone(out, object);
            break;

        default:
            lai_warn("lai_obj_to_hex_string() unsupported object type %d", object->type);
            return LAI_ERROR_ILLEGAL_ARGUMENTS;
    }

    return LAI_ERROR_NONE;
}

lai_api_error_t lai_mutate_string(lai_variable_t *target, lai_variable_t *object) {
    // Strings are resized during mutation.

    switch (object->type) {
        // No conversion necessary.
        case LAI_TYPE_STRING: {
            size_t length = lai_strlen(lai_exec_string_access(object));
            if (lai_obj_resize_string(target, length))
                lai_panic("could not resize string in lai_mutate_string()");
            lai_strcpy(lai_exec_string_access(target), lai_exec_string_access(object));
            break;
        }

        case LAI_TYPE_INTEGER: {
            // Need space for 16 hex digits + one null-terminator.
            // TODO: This depends on the integer width.
            if (lai_obj_resize_string(target, 17))
                lai_panic("could not resize string in lai_mutate_string()");
            char *s = lai_exec_string_access(target);

            lai_snprintf(s, 17, "%016lX", object->integer);
            break;
        }
        case LAI_TYPE_BUFFER: {
            size_t length = lai_exec_buffer_size(object);
            uint8_t *p = lai_exec_buffer_access(object);

            // Need space for '0x12 ' + one null-terminator.
            if (lai_obj_resize_string(target, 5 * length + 1))
                lai_panic("could not resize string in lai_exec_mutate_ns()");
            char *s = lai_exec_string_access(target);

            for (size_t i = 0; i < length; i++) {
                if (!i) {
                    lai_snprintf(s, 5, "0x%02X", p[i]);
                    s += 4;
                } else {
                    lai_snprintf(s, 6, " 0x%02X", p[i]);
                    s += 5;
                }
            }
            *s = '\0';
            break;
        }

        default:
            lai_warn("lai_mutate_string() unsupported object type %d", object->type);
            return LAI_ERROR_ILLEGAL_ARGUMENTS;
    }

    return LAI_ERROR_NONE;
}

lai_api_error_t lai_obj_to_integer(lai_variable_t *out, lai_variable_t *object) {
    switch (object->type) {

        case LAI_BUFFER: {
            size_t buffer_len = lai_exec_buffer_size(object);
            uint64_t *buffer = lai_exec_buffer_access(object);

            if (buffer_len < 8) {
                lai_warn("lai_obj_to_integer() buffer shorter than 8 bytes");
                return LAI_ERROR_ILLEGAL_ARGUMENTS;
            }

            out->type = LAI_INTEGER;
            out->integer = *buffer;

#ifndef __BYTE_ORDER__
#error Required macro __BYTE_ORDER__ not defined
#endif
#if __BYTE_ORDER__ == __ORDER_BIG_ENDIAN__
            out->integer = bswap64(out->integer);
#endif

            break;
        }

        case LAI_STRING: {
            size_t string_len = lai_exec_string_length(object);
            const char *string = lai_exec_string_access(object);

            uint64_t integer = 0;

            // Check if hexadecimal
            if (string_len >= 2 && string[0] == '0' && (string[1] == 'x' || string[1] == 'X')) {
                for (size_t i = 2; i < string_len; i++) {
                    unsigned v;
                    if (string[i] >= '0' && string[i] <= '9')
                        v = string[i] - '0';
                    else if (string[i] >= 'a' && string[i] <= 'f')
                        v = string[i] - 'a' + 10;
                    else if (string[i] >= 'A' && string[i] <= 'F')
                        v = string[i] - 'A' + 10;
                    else {
                        lai_warn("lai_obj_to_integer() hexadecimal string contains non valid "
                                 "character %c",
                                 string[i]);
                        return LAI_ERROR_ILLEGAL_ARGUMENTS;
                    }
                    integer = integer * 16 + v;
                }
            } else {
                for (size_t i = 0; i < string_len; i++) {
                    if (string[i] < '0' || string[i] > '9') {
                        lai_warn(
                            "lai_obj_to_integer() decimal string contains non valid character %c",
                            string[i]);
                        return LAI_ERROR_ILLEGAL_ARGUMENTS;
                    }
                    integer = integer * 10 + (string[i] - '0');
                }
            }

            out->type = LAI_INTEGER;
            out->integer = integer;

            break;
        }

        case LAI_INTEGER:
            lai_obj_clone(out, object);
            break;

        default:
            lai_warn("lai_obj_to_integer() unsupported object type %d", object->type);
            return LAI_ERROR_ILLEGAL_ARGUMENTS;
    }

    return LAI_ERROR_NONE;
}

/*
 * Convert an other than a Buffer, a String or an Integer
 * into a string.
 *
 * This function follows ACPICA's implementation instead
 * of the ACPI standard one.
 */
lai_api_error_t lai_obj_to_type_string(lai_variable_t *target, lai_nsnode_t *object) {
    lai_debug("%d", object->type);
    lai_api_error_t error;
    switch (object->type) {
        case LAI_NAMESPACE_FIELD: {
            error = lai_create_string(target, 14);
            char *str = lai_exec_string_access(target);
            lai_strcpy(str, "[Field Object]");
            break;
        }
        case LAI_NAMESPACE_DEVICE: {
            error = lai_create_string(target, 15);
            char *str = lai_exec_string_access(target);
            lai_strcpy(str, "[Device Object]");
            break;
        }
        case LAI_NAMESPACE_EVENT: {
            error = lai_create_string(target, 14);
            char *str = lai_exec_string_access(target);
            lai_strcpy(str, "[Event Object]");
            break;
        }
        case LAI_NAMESPACE_MUTEX: {
            error = lai_create_string(target, 14);
            char *str = lai_exec_string_access(target);
            lai_strcpy(str, "[Mutex Object]");
            break;
        }
        case LAI_NAMESPACE_OPREGION: {
            error = lai_create_string(target, 15);
            char *str = lai_exec_string_access(target);
            lai_strcpy(str, "[Region Object]");
            break;
        }
        case LAI_NAMESPACE_POWERRESOURCE: {
            error = lai_create_string(target, 14);
            char *str = lai_exec_string_access(target);
            lai_strcpy(str, "[Power Object]");
            break;
        }
        case LAI_NAMESPACE_PROCESSOR: {
            error = lai_create_string(target, 18);
            char *str = lai_exec_string_access(target);
            lai_strcpy(str, "[Processor Object]");
            break;
        }
        case LAI_NAMESPACE_THERMALZONE: {
            error = lai_create_string(target, 14);
            char *str = lai_exec_string_access(target);
            lai_strcpy(str, "[Thermal Zone]");
            break;
        }
        default: {
            lai_warn("lai_obj_to_type_string() unsupported object type %d", object->type);
            return LAI_ERROR_ILLEGAL_ARGUMENTS;
        }
    }
    if (error != LAI_ERROR_NONE) {
        return LAI_ERROR_OUT_OF_MEMORY;
    }
    return LAI_ERROR_NONE;
}

lai_api_error_t lai_mutate_integer(lai_variable_t *target, lai_variable_t *object) {
    switch (object->type) {
        // No conversion necessary.
        case LAI_INTEGER:
            lai_var_assign(target, object);
            break;

        case LAI_STRING: {
            const char *s = lai_exec_string_access(object);
            LAI_ENSURE(target->type == LAI_INTEGER);
            target->integer = 0;

            for (int i = 0; i < 16; i++) {
                if (s[i] >= '0' && s[i] <= '9') {
                    target->integer <<= 4;
                    target->integer |= s[i] - '0';
                } else if (s[i] >= 'a' && s[i] <= 'f') {
                    target->integer <<= 4;
                    target->integer |= (s[i] - 'a') + 10;
                } else if (s[i] >= 'A' && s[i] <= 'F') {
                    target->integer <<= 4;
                    target->integer |= (s[i] - 'A') + 10;
                } else
                    break;
            }
            break;
        }
        case LAI_BUFFER: {
            LAI_ENSURE(target->type == LAI_INTEGER);
            target->integer = 0;

            // TODO: This assumes that 64-bit integers are used.
            size_t copy_size = lai_exec_buffer_size(object);
            if (copy_size > 8)
                copy_size = 8;
            memcpy(&target->integer, lai_exec_buffer_access(object), copy_size);
            // TODO: bswap() if necessary.
            break;
        }

        default:
            lai_warn("lai_mutate_integer() unsupported object type %d", object->type);
            return LAI_ERROR_ILLEGAL_ARGUMENTS;
    }

    return LAI_ERROR_NONE;
}

// lai_clone_buffer(): Clones a buffer object
static void lai_clone_buffer(lai_variable_t *dest, lai_variable_t *source) {
    size_t size = lai_exec_buffer_size(source);
    if (lai_create_buffer(dest, size) != LAI_ERROR_NONE)
        lai_panic("unable to allocate memory for buffer object.");
    memcpy(lai_exec_buffer_access(dest), lai_exec_buffer_access(source), size);
}

// lai_clone_string(): Clones a string object
static void lai_clone_string(lai_variable_t *dest, lai_variable_t *source) {
    size_t n = lai_exec_string_length(source);
    if (lai_create_string(dest, n) != LAI_ERROR_NONE)
        lai_panic("unable to allocate memory for string object.");
    memcpy(lai_exec_string_access(dest), lai_exec_string_access(source), n);
}

// lai_clone_package(): Clones a package object
static void lai_clone_package(lai_variable_t *dest, lai_variable_t *src) {
    size_t n = src->pkg_ptr->size;
    if (lai_create_pkg(dest, n) != LAI_ERROR_NONE)
        lai_panic("unable to allocate memory for package object.");
    for (size_t i = 0; i < n; i++)
        lai_obj_clone(&dest->pkg_ptr->elems[i], &src->pkg_ptr->elems[i]);
}

extern void lai_swap_object(lai_variable_t *first, lai_variable_t *second); // from core/variable.c

// lai_obj_clone(): Copies an object
void lai_obj_clone(lai_variable_t *dest, lai_variable_t *source) {
    // Clone into a temporary object.
    lai_variable_t temp = {0};
    switch (source->type) {
        case LAI_STRING:
            lai_clone_string(&temp, source);
            break;
        case LAI_BUFFER:
            lai_clone_buffer(&temp, source);
            break;
        case LAI_PACKAGE:
            lai_clone_package(&temp, source);
            break;
    }

    if (temp.type) {
        // Afterwards, swap to the destination. This handles copy-to-self correctly.
        lai_swap_object(dest, &temp);
        lai_var_finalize(&temp);
    } else {
        // For others objects: just do a shallow copy.
        lai_var_assign(dest, source);
    }
}

int lai_objecttype_obj(lai_variable_t *var) {
    switch (var->type) {
        case LAI_INTEGER:
            return 1;
        case LAI_STRING_INDEX:
        case LAI_STRING:
            return 2;
        case LAI_BUFFER_INDEX:
        case LAI_BUFFER:
            return 3;
        case LAI_PACKAGE_INDEX:
        case LAI_PACKAGE:
            return 4;
        default:
            return 0;
    }
}

int lai_objecttype_ns(lai_nsnode_t *node) {
    switch (node->type) {
        case LAI_NAMESPACE_NAME:
            return lai_objecttype_obj(&node->object);
        case LAI_NAMESPACE_FIELD:
        case LAI_NAMESPACE_BANKFIELD:
        case LAI_NAMESPACE_INDEXFIELD:
            return 5;
        case LAI_NAMESPACE_DEVICE:
            return 6;
        case LAI_NAMESPACE_EVENT:
            return 7;
        case LAI_NAMESPACE_METHOD:
            return 8;
        case LAI_NAMESPACE_MUTEX:
            return 9;
        case LAI_NAMESPACE_OPREGION:
            return 10;
        case LAI_NAMESPACE_POWERRESOURCE:
            return 11;
        case LAI_NAMESPACE_PROCESSOR:
            return 12;
        case LAI_NAMESPACE_THERMALZONE:
            return 13;
        case LAI_NAMESPACE_BUFFER_FIELD:
            return 14;
        default:
            break;
    }
    return 0;
}

lai_api_error_t lai_obj_exec_match_op(int op, lai_variable_t *var, lai_variable_t *obj, int *out) {
    LAI_CLEANUP_VAR lai_variable_t compare_obj = LAI_VAR_INITIALIZER;
    int result = 0;

    if (var->type == LAI_INTEGER) {
        lai_api_error_t err = lai_obj_to_integer(&compare_obj, obj);
        if (err != LAI_ERROR_NONE)
            return err;

        switch (op) {
            case MATCH_MTR: // MTR: Always True
                result = 1;
                break;
            case MATCH_MEQ: // MEQ: Equals
                result = (var->integer == compare_obj.integer);
                break;
            case MATCH_MLE: // MLE: Less than or equal
                result = (var->integer <= compare_obj.integer);
                break;
            case MATCH_MLT: // MLT: Less than
                result = (var->integer < compare_obj.integer);
                break;
            case MATCH_MGE: // MGE: Greater than or equal
                result = (var->integer >= compare_obj.integer);
                break;
            case MATCH_MGT: // MGT: Greater than
                result = (var->integer > compare_obj.integer);
                break;

            default:
                lai_warn("lai_obj_exec_match_op: Illegal op passed %d", op);
                return LAI_ERROR_UNEXPECTED_RESULT;
        }
    } else if (var->type == LAI_BUFFER || var->type == LAI_STRING) {
        char *var_data = NULL;
        char *obj_data = NULL;

        size_t var_size = 0;
        size_t obj_size = 0;

        if (var->type == LAI_BUFFER) {
            lai_api_error_t err = lai_obj_to_buffer(&compare_obj, obj);
            if (err != LAI_ERROR_NONE)
                return err;

            var_data = lai_exec_buffer_access(var);
            obj_data = lai_exec_buffer_access(&compare_obj);

            var_size = lai_exec_buffer_size(var);
            obj_size = lai_exec_buffer_size(&compare_obj);
        } else {
            lai_api_error_t err = lai_obj_to_hex_string(&compare_obj, obj);
            if (err != LAI_ERROR_NONE)
                return err;

            var_data = lai_exec_string_access(var);
            obj_data = lai_exec_string_access(&compare_obj);

            var_size = lai_exec_string_length(var);
            obj_size = lai_exec_string_length(&compare_obj);
        }

        int compare = memcmp(var_data, obj_data, (var_size > obj_size) ? obj_size : var_size);

        switch (op) {
            case MATCH_MTR: // MTR: Always True
                result = 1;
                break;
            case MATCH_MEQ: // MEQ: Equals
                result = (compare == 0 && var_size == obj_size);
                break;
            case MATCH_MLE: // MLE: Less than or equal
                if (compare == 0) {
                    result = var_size > obj_size;
                } else {
                    result = (compare > 0);
                }

                result = !result; // (a <= b) = !(a > b)
                break;
            case MATCH_MLT: // MLT: Less than
                if (compare == 0) {
                    result = var_size < obj_size;
                } else {
                    result = (compare < 0);
                }

                break;
            case MATCH_MGE: // MGE: Greater than or equal
                if (compare == 0) {
                    result = var_size < obj_size;
                } else {
                    result = (compare < 0);
                }

                result = !result; // (a >= 0) = !(a < b);
                break;
            case MATCH_MGT: // MGT: Greater than
                if (compare == 0) {
                    result = var_size > obj_size;
                } else {
                    result = (compare > 0);
                }

                break;
            default:
                lai_warn("lai_obj_exec_match_op: Illegal op passed %d", op);
                return LAI_ERROR_UNEXPECTED_RESULT;
        }
    } else {
        lai_warn("lai_obj_exec_match_op: Illegal object type passed %d", var->type);
        return LAI_ERROR_UNEXPECTED_RESULT;
    }

    *out = result;

    return LAI_ERROR_NONE;
}

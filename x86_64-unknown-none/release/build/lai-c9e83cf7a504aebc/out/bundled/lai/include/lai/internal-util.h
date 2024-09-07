/*
 * Lightweight AML Interpreter
 * Copyright (C) 2018-2023 The lai authors
 */

#pragma once

#include <lai/host.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

size_t lai_strlen(const char *);

// Even in freestanding environments, GCC requires memcpy(), memmove(), memset()
// and memcmp() to be present. Thus, we just use them directly.
void *memcpy(void *, const void *, size_t);
void *memmove(void *, const void *, size_t);
void *memset(void *, int, size_t);
int memcmp(const void *, const void *, size_t);

//---------------------------------------------------------------------------------------
// Debugging and logging functions.
//---------------------------------------------------------------------------------------

__attribute__((format(printf, 1, 2))) void lai_debug(const char *, ...);
__attribute__((format(printf, 1, 2))) void lai_warn(const char *, ...);
__attribute__((noreturn, format(printf, 1, 2))) void lai_panic(const char *, ...);

#define LAI_STRINGIFY(x) #x
#define LAI_EXPAND_STRINGIFY(x) LAI_STRINGIFY(x)

#define LAI_ENSURE(cond)                                                                           \
    do {                                                                                           \
        if (!(cond))                                                                               \
            lai_panic("assertion failed: " #cond " at " __FILE__                                   \
                      ":" LAI_EXPAND_STRINGIFY(__LINE__) "\n");                                    \
    } while (0)

#define LAI_TRY(expr)                                                                              \
    ({                                                                                             \
        lai_api_error_t try_res_ = (expr);                                                         \
        if (try_res_ != LAI_ERROR_NONE)                                                            \
            return try_res_;                                                                       \
        try_res_;                                                                                  \
    })

//---------------------------------------------------------------------------------------
// Misc. utility functions.
//---------------------------------------------------------------------------------------

static inline void lai_cleanup_free_string(char **v) {
    if (*v)
        laihost_free(*v, lai_strlen(*v) + 1);
}

#define LAI_CLEANUP_FREE_STRING __attribute__((cleanup(lai_cleanup_free_string)))

//---------------------------------------------------------------------------------------
// Reference counting functions.
//---------------------------------------------------------------------------------------

typedef int lai_rc_t;

__attribute__((always_inline)) inline void lai_rc_ref(lai_rc_t *rc_ptr) {
    lai_rc_t nrefs = (*rc_ptr)++;
    LAI_ENSURE(nrefs > 0);
}

__attribute__((always_inline)) inline int lai_rc_unref(lai_rc_t *rc_ptr) {
    lai_rc_t nrefs = --(*rc_ptr);
    LAI_ENSURE(nrefs >= 0);
    return !nrefs;
}

//---------------------------------------------------------------------------------------
// List data structure.
//---------------------------------------------------------------------------------------

struct lai_list_item {
    struct lai_list_item *next;
    struct lai_list_item *prev;
};

struct lai_list {
    struct lai_list_item hook;
};

//---------------------------------------------------------------------------------------
// Hash table data structure.
//---------------------------------------------------------------------------------------

struct lai_hashtable {
    int elem_capacity; // Capacity of elem_{ptr,hash}_tab.
    int bucket_capacity; // Size of bucket_tab. *Must* be a power of 2.
    int num_elems; // Number of elements in the table.
    void **elem_ptr_tab; // Stores the pointer of each element.
    int *elem_hash_tab; // Stores the hash of each element.
    int *bucket_tab; // Indexes into elem_{ptr,hash}_tab.
};

#ifdef __cplusplus
}
#endif

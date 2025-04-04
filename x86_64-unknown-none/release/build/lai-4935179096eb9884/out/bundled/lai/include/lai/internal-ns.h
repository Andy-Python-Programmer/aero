/*
 * Lightweight AML Interpreter
 * Copyright (C) 2018-2023 The lai authors
 */

#pragma once

#include <acpispec/tables.h>
#include <lai/error.h>
#include <lai/host.h>
#include <lai/internal-exec.h>
#include <lai/internal-util.h>

#ifdef __cplusplus
extern "C" {
#endif

__attribute__((always_inline)) inline void lai_namecpy(char *dest, const char *src) {
    memcpy(dest, src, 4);
}

struct lai_aml_segment {
    acpi_aml_t *table;
    // Index of the table (e.g., for SSDTs).
    size_t index;
};

struct lai_opregion_override {
    uint8_t (*readb)(uint64_t, void *);
    uint16_t (*readw)(uint64_t, void *);
    uint32_t (*readd)(uint64_t, void *);
    uint64_t (*readq)(uint64_t, void *);

    void (*writeb)(uint64_t, uint8_t, void *);
    void (*writew)(uint64_t, uint16_t, void *);
    void (*writed)(uint64_t, uint32_t, void *);
    void (*writeq)(uint64_t, uint64_t, void *);
};

enum lai_node_type {
    LAI_NODETYPE_NULL,
    LAI_NODETYPE_ROOT,
    LAI_NODETYPE_EVALUATABLE,
    LAI_NODETYPE_DEVICE,
    LAI_NODETYPE_MUTEX,
    LAI_NODETYPE_PROCESSOR,
    LAI_NODETYPE_THERMALZONE,
    LAI_NODETYPE_EVENT,
    LAI_NODETYPE_POWERRESOURCE,
    LAI_NODETYPE_OPREGION,
};

#define LAI_NAMESPACE_ROOT 1
#define LAI_NAMESPACE_NAME 2
#define LAI_NAMESPACE_ALIAS 3
#define LAI_NAMESPACE_FIELD 4
#define LAI_NAMESPACE_METHOD 5
#define LAI_NAMESPACE_DEVICE 6
#define LAI_NAMESPACE_INDEXFIELD 7
#define LAI_NAMESPACE_MUTEX 8
#define LAI_NAMESPACE_PROCESSOR 9
#define LAI_NAMESPACE_BUFFER_FIELD 10
#define LAI_NAMESPACE_THERMALZONE 11
#define LAI_NAMESPACE_EVENT 12
#define LAI_NAMESPACE_POWERRESOURCE 13
#define LAI_NAMESPACE_BANKFIELD 14
#define LAI_NAMESPACE_OPREGION 15

typedef struct lai_nsnode {
    char name[4];
    int type;
    struct lai_nsnode *parent;
    struct lai_aml_segment *amls;
    void *pointer; // valid for scopes, methods, etc.
    size_t size; // valid for scopes, methods, etc.

    lai_variable_t object; // for Name()

    // Implements the Notify() AML operator.
    lai_api_error_t (*notify_override)(struct lai_nsnode *, int, void *);
    void *notify_userptr;

    uint8_t method_flags; // for Methods only, includes ARG_COUNT in lowest three bits
    // Allows the OS to override methods. Mainly useful for _OSI, _OS and _REV.
    int (*method_override)(lai_variable_t *args, lai_variable_t *result);

    // TODO: Find a good mechanism for locks.
    // lai_lock_t mutex;        // for Mutex

    union {
        struct lai_nsnode *al_target; // LAI_NAMESPACE_ALIAS.

        struct { // LAI_NAMESPACE_FIELD and LAI_NAMESPACE_BANK_FIELD and LAI_NAMESPACE_INDEX_FIELD
            struct lai_nsnode *fld_region_node;
            uint64_t fld_offset; // In bits.
            size_t fld_size; // In bits.
            uint8_t fld_flags;

            union {
                struct {
                    struct lai_nsnode *fld_bkf_bank_node;
                    uint64_t fld_bkf_value;
                };

                struct {
                    struct lai_nsnode *fld_idxf_index_node;
                    struct lai_nsnode *fld_idxf_data_node;
                };
            };
        };
        struct { // LAI_NAMESPACE_BUFFER_FIELD.
            struct lai_buffer_head *bf_buffer;
            uint64_t bf_offset; // In bits.
            uint64_t bf_size; // In bits.
        };
        struct { // LAI_NAMESPACE_PROCESSOR
            uint8_t cpu_id;
            uint32_t pblk_addr;
            uint8_t pblk_len;
        };
        struct { // LAI_NAMESPACE_OPREGION
            uint8_t op_address_space;
            uint64_t op_base;
            uint64_t op_length;
            const struct lai_opregion_override *op_override;
            void *op_userptr;
        };
        struct { // LAI_NAMESPACE_MUTEX
            struct lai_sync_state mut_sync;
        };
        struct { // LAI_NAMESPACE_EVENT
            struct lai_sync_state evt_sync;
        };
    };

    // Stores a list of all namespace nodes created by the same method.
    struct lai_list_item per_method_item;

    // Hash table that stores the children of each node.
    struct lai_hashtable children;
} lai_nsnode_t;

#ifdef __cplusplus
}
#endif

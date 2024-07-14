/*
 * Lightweight AML Interpreter
 * Copyright (C) 2018-2023 The lai authors
 */

#pragma once

#include <lai/core.h>
#include <lai/helpers/resource.h>

#ifdef __cplusplus
extern "C" {
#endif

struct lai_ec_driver {
    uint16_t cmd_port;
    uint16_t data_port;
};

#ifdef __cplusplus
#define LAI_EC_DRIVER_INITIALIZER                                                                  \
    {}
#else
#define LAI_EC_DRIVER_INITIALIZER                                                                  \
    { 0 }
#endif

static inline void lai_initialize_ec_driver(struct lai_ec_driver *ec) {
    *ec = (struct lai_ec_driver)LAI_EC_DRIVER_INITIALIZER;
}

void lai_early_init_ec(struct lai_ec_driver *);
void lai_init_ec(lai_nsnode_t *, struct lai_ec_driver *);
uint8_t lai_read_ec(uint8_t, struct lai_ec_driver *);
void lai_write_ec(uint8_t, uint8_t, struct lai_ec_driver *);
uint8_t lai_query_ec(struct lai_ec_driver *);

extern const struct lai_opregion_override lai_ec_opregion_override;

#ifdef __cplusplus
}
#endif

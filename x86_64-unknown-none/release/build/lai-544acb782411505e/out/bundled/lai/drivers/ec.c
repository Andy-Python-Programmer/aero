/*
 * Lightweight AML Interpreter
 * Copyright (C) 2018-2023 The lai authors
 */

/* LAI Embedded Controller implementation
 * ACPI 6.3 Specification chapter 12
 * ACPI defines an Embedded Controller for interfacing directly with firmware
 */

#include <lai/drivers/ec.h>

void lai_early_init_ec(struct lai_ec_driver *driver) {
    if (!laihost_scan)
        lai_panic("host does not implement laihost_scan required for lai_early_init_ec");

    acpi_ecdt_t *ecdt = laihost_scan(ACPI_ECDT_ID, 0);
    if (!ecdt) {
        lai_warn("Couldn't find ECDT for initializing EC");
        return;
    }

    // TODO: Support MMIO like the spec states
    if (ecdt->ec_control.address_space != ACPI_GAS_IO) {
        lai_warn("Unsupported ECDT Command address space %02X", ecdt->ec_control.address_space);
        return;
    }
    driver->cmd_port = ecdt->ec_control.base;

    if (ecdt->ec_data.address_space != ACPI_GAS_IO) {
        lai_warn("Unsupported ECDT Data address space %02X", ecdt->ec_data.address_space);
        return;
    }
    driver->data_port = ecdt->ec_data.base;
}

void lai_init_ec(lai_nsnode_t *node, struct lai_ec_driver *driver) {
    LAI_CLEANUP_STATE lai_state_t state;
    lai_init_state(&state);

    LAI_CLEANUP_VAR lai_variable_t pnp_id = LAI_VAR_INITIALIZER;
    lai_eisaid(&pnp_id, ACPI_EC_PNP_ID);

    if (lai_check_device_pnp_id(node, &pnp_id, &state)) {
        lai_warn("node supplied to lai_init_ec() is not an Embedded Controller");
        return;
    }

    // Found an EC
    lai_nsnode_t *crs_node = lai_resolve_path(node, "_CRS");
    if (!crs_node) {
        lai_warn("Couldn't find _CRS for initializing EC driver");
        return;
    }

    LAI_CLEANUP_VAR lai_variable_t crs = LAI_VAR_INITIALIZER;
    if (lai_eval(&crs, crs_node, &state)) {
        lai_warn("Couldn't eval _CRS for initializing EC driver");
        return;
    }

    struct lai_resource_view crs_it = LAI_RESOURCE_VIEW_INITIALIZER(&crs);
    lai_api_error_t error;

    error = lai_resource_iterate(&crs_it);
    if (error != LAI_ERROR_NONE) {
        lai_warn("Encountered error while iterating EC _CRS: %s", lai_api_error_to_string(error));
        return;
    }
    enum lai_resource_type type = lai_resource_get_type(&crs_it);
    if (type != LAI_RESOURCE_IO) {
        lai_warn("Unknown resource type while iterating EC _CRS: %02X", type);
        return;
    }
    driver->data_port = crs_it.base;

    error = lai_resource_iterate(&crs_it);
    if (error == LAI_ERROR_END_REACHED) {
        // TODO: Support Hardware reduced ACPI systems
        return;
    } else if (error != LAI_ERROR_NONE) {
        lai_warn("Encountered error while iterating EC _CRS: %s", lai_api_error_to_string(error));
        return;
    }
    type = lai_resource_get_type(&crs_it);
    if (type != LAI_RESOURCE_IO) {
        lai_warn("Unknown resource type while iterating EC _CRS: %02X", type);
        return;
    }
    driver->cmd_port = crs_it.base;
}

static void poll_ibf(struct lai_ec_driver *driver) {
    while (1) {
        uint8_t status = laihost_inb(driver->cmd_port);
        if ((status & ACPI_EC_STATUS_IBF) == 0)
            return;
    }
}

static void poll_obf(struct lai_ec_driver *driver) {
    while (1) {
        uint8_t status = laihost_inb(driver->cmd_port);
        if ((status & ACPI_EC_STATUS_OBF) != 0)
            return;
    }
}

/* While the EC is in burst mode it won't generate any SMIs or SCIs that aren't critical
 * This is to keep the speed of the operation up and to keep the EC state consistent while we are
 * working However disabling interrupts or anything to guarantee that nothing bothers us while
 * working with the EC is not neccesary - since the EC will automatically drop out of Burst mode
 * (See ACPI 6.3 Specification 12.3.3) if it has been idle for too long - or has remained in burst
 * mode for too long.
 */
static void enable_burst(struct lai_ec_driver *driver) {
    laihost_outb(driver->cmd_port, ACPI_EC_BURST_ENABLE); // Spec specifies that no interrupt will
                                                          // be generated for this command
    poll_obf(driver);
    if (laihost_inb(driver->data_port) != 0x90)
        lai_panic("Enabling EC Burst Mode Failed");

    // According to the spec ACPI_EC_STATUS_BURST should get set, but it has been observed that it
    // doesn't on real HW. Linux also doesn't check that it gets set
}

static void disable_burst(struct lai_ec_driver *driver) {
    poll_ibf(driver);
    laihost_outb(driver->cmd_port, ACPI_EC_BURST_DISABLE);
    while ((laihost_inb(driver->cmd_port) & ACPI_EC_STATUS_BURST) != 0)
        ;
}

uint8_t lai_read_ec(uint8_t offset, struct lai_ec_driver *driver) {
    if (driver->cmd_port == 0 || driver->data_port == 0) {
        lai_warn("EC driver has not yet been initialized");
        return 0;
    }

    if (!laihost_outb || !laihost_inb)
        lai_panic("host does not provide io functions required by lai_read_ec()");

    poll_ibf(driver);
    laihost_outb(driver->cmd_port, ACPI_EC_READ);

    poll_ibf(driver);
    laihost_outb(driver->data_port, offset);

    poll_obf(driver);
    uint8_t ret = laihost_inb(driver->data_port);

    return ret;
}

void lai_write_ec(uint8_t offset, uint8_t value, struct lai_ec_driver *driver) {
    if (driver->cmd_port == 0 || driver->data_port == 0) {
        lai_warn("EC driver has not yet been initialized");
        return;
    }

    if (!laihost_outb || !laihost_inb)
        lai_panic("host does not provide io functions required by lai_read_ec()");

    poll_ibf(driver);
    laihost_outb(driver->cmd_port, ACPI_EC_WRITE);

    poll_ibf(driver);
    laihost_outb(driver->data_port, offset);

    poll_ibf(driver);
    laihost_outb(driver->data_port, value);
}

uint8_t lai_query_ec(struct lai_ec_driver *driver) {
    if (driver->cmd_port == 0 || driver->data_port == 0) {
        lai_warn("EC driver has not yet been initialized");
        return 0;
    }

    if (!laihost_outb || !laihost_inb)
        lai_panic("host does not provide io functions required by lai_read_ec()");

    enable_burst(driver);

    laihost_outb(
        driver->cmd_port,
        ACPI_EC_QUERY); // Spec specifies that no interrupt will be generated for this command
    poll_obf(driver);

    disable_burst(driver);
    return laihost_inb(driver->data_port);
}

static uint8_t readb(uint64_t offset, void *userptr) {
    enable_burst(userptr);
    uint8_t ret = lai_read_ec(offset, userptr);
    disable_burst(userptr);
    return ret;
}

static uint16_t readw(uint64_t offset, void *userptr) {
    enable_burst(userptr);
    uint16_t ret = (uint16_t)lai_read_ec(offset, userptr)
                   | ((uint16_t)lai_read_ec(offset + 1, userptr) << 8);
    disable_burst(userptr);
    return ret;
}

static uint32_t readd(uint64_t offset, void *userptr) {
    enable_burst(userptr);
    uint32_t ret = (uint32_t)lai_read_ec(offset, userptr)
                   | ((uint32_t)lai_read_ec(offset + 1, userptr) << 8)
                   | ((uint32_t)lai_read_ec(offset + 2, userptr) << 16)
                   | ((uint32_t)lai_read_ec(offset + 3, userptr) << 24);
    disable_burst(userptr);
    return ret;
}

static uint64_t readq(uint64_t offset, void *userptr) {
    enable_burst(userptr);
    uint64_t ret = lai_read_ec(offset, userptr) | ((uint64_t)lai_read_ec(offset + 1, userptr) << 8)
                   | ((uint64_t)lai_read_ec(offset + 2, userptr) << 16)
                   | ((uint64_t)lai_read_ec(offset + 3, userptr) << 24)
                   | ((uint64_t)lai_read_ec(offset + 4, userptr) << 32)
                   | ((uint64_t)lai_read_ec(offset + 5, userptr) << 40)
                   | ((uint64_t)lai_read_ec(offset + 6, userptr) << 48)
                   | ((uint64_t)lai_read_ec(offset + 7, userptr) << 56);
    disable_burst(userptr);
    return ret;
}

static void writeb(uint64_t offset, uint8_t value, void *userptr) {
    enable_burst(userptr);
    lai_write_ec(offset, value, userptr);
    disable_burst(userptr);
}

static void writew(uint64_t offset, uint16_t value, void *userptr) {
    enable_burst(userptr);
    lai_write_ec(offset, value & 0xFF, userptr);
    lai_write_ec(offset + 1, (value >> 8) & 0xFF, userptr);
    disable_burst(userptr);
}

static void writed(uint64_t offset, uint32_t value, void *userptr) {
    enable_burst(userptr);
    lai_write_ec(offset, value & 0xFF, userptr);
    lai_write_ec(offset + 1, (value >> 8) & 0xFF, userptr);
    lai_write_ec(offset + 2, (value >> 16) & 0xFF, userptr);
    lai_write_ec(offset + 3, (value >> 24) & 0xFF, userptr);
    disable_burst(userptr);
}

static void writeq(uint64_t offset, uint64_t value, void *userptr) {
    enable_burst(userptr);
    lai_write_ec(offset, value & 0xFF, userptr);
    lai_write_ec(offset + 1, (value >> 8) & 0xFF, userptr);
    lai_write_ec(offset + 2, (value >> 16) & 0xFF, userptr);
    lai_write_ec(offset + 3, (value >> 24) & 0xFF, userptr);
    lai_write_ec(offset + 4, (value >> 32) & 0xFF, userptr);
    lai_write_ec(offset + 5, (value >> 40) & 0xFF, userptr);
    lai_write_ec(offset + 6, (value >> 48) & 0xFF, userptr);
    lai_write_ec(offset + 7, (value >> 56) & 0xFF, userptr);
    disable_burst(userptr);
}

const struct lai_opregion_override lai_ec_opregion_override = {.readb = readb,
                                                               .readw = readw,
                                                               .readd = readd,
                                                               .readq = readq,
                                                               .writeb = writeb,
                                                               .writew = writew,
                                                               .writed = writed,
                                                               .writeq = writeq};

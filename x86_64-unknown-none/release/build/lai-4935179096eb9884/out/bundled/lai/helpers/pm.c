/*
 * Lightweight AML Interpreter
 * Copyright (C) 2018-2023 The lai authors
 */

/* Sleeping Functions */
/* As of now, it's only for S5 (shutdown) sleep, because other sleeping states
 * need NVS and other things I still need to research */

#include <lai/helpers/pm.h>

#include "../core/eval.h"
#include "../core/libc.h"

// lai_enter_sleep(): Enters a sleeping state, note: currently only works for S5
// Param:    uint8_t state - 0-5 to correspond with states S0-S5
// Return:    int - 0 on success
lai_api_error_t lai_enter_sleep(uint8_t sleep_state) {
    struct lai_instance *instance = lai_current_instance();

    LAI_CLEANUP_STATE lai_state_t state;
    lai_init_state(&state);

    const char *sleep_object;
    switch (sleep_state) {
        case 0:
            sleep_object = "\\_S0";
            break;
        case 1:
            sleep_object = "\\_S1";
            break;
        case 2:
            sleep_object = "\\_S2";
            break;
        case 3:
            sleep_object = "\\_S3";
            break;
        case 4:
            sleep_object = "\\_S4";
            break;
        case 5:
            sleep_object = "\\_S5";
            break;
        default:
            lai_panic("undefined sleep state S%d", sleep_state);
    }

    // get sleeping package
    lai_nsnode_t *handle = lai_resolve_path(NULL, sleep_object);
    if (!handle) {
        lai_debug("sleep state S%d is not supported.", sleep_state);
        return LAI_ERROR_UNSUPPORTED;
    }

    LAI_CLEANUP_VAR lai_variable_t package = LAI_VAR_INITIALIZER;
    LAI_CLEANUP_VAR lai_variable_t slp_typa = LAI_VAR_INITIALIZER;
    LAI_CLEANUP_VAR lai_variable_t slp_typb = LAI_VAR_INITIALIZER;
    int eval_status;
    eval_status = lai_eval(&package, handle, &state);
    if (eval_status) {
        lai_debug("sleep state S%d is not supported.", sleep_state);
        return LAI_ERROR_UNSUPPORTED;
    }

    lai_debug("entering sleep state S%d...", sleep_state);
    lai_obj_get_pkg(&package, 0, &slp_typa);
    lai_obj_get_pkg(&package, 1, &slp_typb);

    // ACPI spec says we should call _PTS() and _GTS() before actually sleeping
    // Who knows, it might do some required firmware-specific stuff
    handle = lai_resolve_path(NULL, "\\_PTS");

    if (handle) {
        lai_init_state(&state);

        // pass the sleeping type as an argument
        LAI_CLEANUP_VAR lai_variable_t sleep_object = LAI_VAR_INITIALIZER;
        sleep_object.type = LAI_INTEGER;
        sleep_object.integer = sleep_state & 0xFF;

        lai_debug("execute _PTS(%d)", sleep_state);
        lai_eval_largs(NULL, handle, &state, &sleep_object, NULL);
        lai_finalize_state(&state);
    }

    // _GTS has actually become obsolete with ACPI 5.0A
    if (instance->fadt->header.revision < 5) {
        handle = lai_resolve_path(NULL, "\\_GTS");

        if (handle) {
            lai_init_state(&state);

            // pass the sleeping type as an argument
            LAI_CLEANUP_VAR lai_variable_t sleep_object = LAI_VAR_INITIALIZER;
            sleep_object.type = LAI_INTEGER;
            sleep_object.integer = sleep_state & 0xFF;

            lai_debug("execute _GTS(%d)", sleep_state);
            lai_eval_largs(NULL, handle, &state, &sleep_object, NULL);
            lai_finalize_state(&state);
        }
    }

    if (instance->is_hw_reduced) {
        // HW-Reduced systems reuse the SLP_TYPa field for the HW-Reduced sleep type
        acpi_gas_t *control_reg = &instance->fadt->sleep_control_reg;
        uint8_t value = ((slp_typa.integer & 0x7) << 2) | (1 << 5);

        if (!control_reg->base)
            return LAI_ERROR_UNSUPPORTED;

        if (control_reg->bit_width != 8)
            lai_warn("Invalid sleep control register bit width, assuming 8");

        if (control_reg->bit_offset != 0)
            lai_warn("Invalid sleep control register bit offset, assuming 0");

        switch (control_reg->address_space) {
            case ACPI_GAS_IO:
                if (!laihost_outb)
                    lai_panic("lai_enter_sleep() requires port I/O");

                laihost_outb(control_reg->base, value);
                break;
            case ACPI_GAS_MMIO:
                if (!laihost_map)
                    lai_panic("lai_enter_sleep() requires laihost_map()");

                volatile uint8_t *control = laihost_map(control_reg->base, 1);
                *control = value;
                break;
            case ACPI_GAS_PCI:
                if (!laihost_pci_writeb)
                    lai_panic("lai_enter_sleep() requires laihost_pci_writeb()");

                laihost_pci_writeb(0, 0, (control_reg->base >> 32) & 0xFFFF,
                                   (control_reg->base >> 16) & 0xFFFF, control_reg->base & 0xFFFF,
                                   value);
                break;
            default:
                lai_warn("Invalid sleep control register address space: %d",
                         control_reg->address_space);
                return LAI_ERROR_UNEXPECTED_RESULT;
        }
    } else {
        if (!laihost_inw || !laihost_outw)
            lai_panic("lai_enter_sleep() requires port I/O");

        // and go to sleep
        uint16_t data;
        data = laihost_inw(instance->fadt->pm1a_control_block);
        data &= 0xE3FF;
        data |= (slp_typa.integer << 10) | ACPI_SLEEP;
        laihost_outw(instance->fadt->pm1a_control_block, data);

        if (instance->fadt->pm1b_control_block) {
            data = laihost_inw(instance->fadt->pm1b_control_block);
            data &= 0xE3FF;
            data |= (slp_typb.integer << 10) | ACPI_SLEEP;
            laihost_outw(instance->fadt->pm1b_control_block, data);
        }
    }

    return LAI_ERROR_NONE;
}

lai_api_error_t lai_acpi_reset() {
    struct lai_instance *instance = lai_current_instance();
    acpi_fadt_t *fadt = instance->fadt;
    if (!fadt) {
        if (!laihost_scan)
            lai_panic("laihost_scan is required for lai_acpi_reset");

        fadt = laihost_scan("FACP", 0); // If fadt hasn't been initialized yet get it, AML doesn't
                                        // have to be scanned for reset
        if (!fadt)
            lai_panic("Buggy BIOS does not provide FADT");
    }

    uint32_t fixed_flags = fadt->flags;
    if (!(fixed_flags & (1 << 10))) // System doesn't indicate support for ACPI reset via flags
        return LAI_ERROR_UNSUPPORTED;

    switch (fadt->reset_register.address_space) {
        case ACPI_GAS_MMIO: {
            if (!laihost_map)
                lai_panic("laihost_map is required for lai_acpi_reset");
            laihost_map(fadt->reset_register.base, 1); // We only need 1 byte mapped
            uint8_t *reg = (uint8_t *)((uintptr_t)fadt->reset_register.base);
            *reg = fadt->reset_command;
            break;
        }
        case ACPI_GAS_IO:
            if (!laihost_outb)
                lai_panic("laihost_outb is required for lai_acpi_reset");

            laihost_outb(fadt->reset_register.base, fadt->reset_command);
            break;
        case ACPI_GAS_PCI:
            // Spec states that it is at Seg 0, bus 0
            laihost_pci_writeb(0, 0, (fadt->reset_register.base >> 32) & 0xFFFF,
                               (fadt->reset_register.base >> 16) & 0xFFFF,
                               fadt->reset_register.base & 0xFFFF, fadt->reset_command);
            break;
        default:
            lai_panic("Unknown FADT reset reg address space type: 0x%02X",
                      fadt->reset_register.address_space);
    }

    return LAI_ERROR_NONE;
}

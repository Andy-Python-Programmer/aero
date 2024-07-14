/*
 * Lightweight AML Interpreter
 * Copyright (C) 2018-2023 The lai authors
 */

#include <lai/drivers/timer.h>
#include <lai/helpers/sci.h>

// ACPI timer runs at 3.579545 MHz

static acpi_gas_t timer_block;
static volatile uint32_t *timer_mmio_reg = NULL;
static int extended_timer = 0;
static int supported = 0;

uint32_t lai_read_pm_timer_value() {
    if (timer_block.address_space == ACPI_GAS_IO) {
        return laihost_ind(timer_block.base);
    } else if (timer_block.address_space == ACPI_GAS_MMIO) {
        return *timer_mmio_reg;
    } else {
        lai_panic("Unknown ACPI Timer address space");
    }

    return 0;
}

lai_api_error_t lai_start_pm_timer() {
    acpi_fadt_t *fadt = lai_current_instance()->fadt;

    if (fadt->pm_timer_length != 4)
        return LAI_ERROR_UNSUPPORTED;

    supported = 1;

    if (lai_current_instance()->acpi_revision >= 2 && fadt->x_pm_timer_block.base) {
        timer_block = fadt->x_pm_timer_block;
        if (timer_block.address_space == ACPI_GAS_MMIO)
            timer_mmio_reg = (volatile uint32_t *)laihost_map(timer_block.base, 4);
    } else {
        timer_block.address_space = ACPI_GAS_IO;
        timer_block.base = fadt->pm_timer_block;
    }

    if (fadt->flags & (1 << 8))
        extended_timer = 1;

    lai_set_sci_event(lai_get_sci_event() | ACPI_TIMER);

    return LAI_ERROR_NONE;
}

lai_api_error_t lai_stop_pm_timer() {
    if (!supported)
        return LAI_ERROR_UNSUPPORTED;

    lai_set_sci_event(lai_get_sci_event() & ~ACPI_TIMER);
    return LAI_ERROR_NONE;
}

lai_api_error_t lai_busy_wait_pm_timer(uint64_t ms) {
    if (!supported)
        return LAI_ERROR_UNSUPPORTED;

    // number of ticks per millisecond 3579.545, rounded up to 3580
    uint32_t goal = lai_read_pm_timer_value() + (ms * 3580);

    if (!extended_timer && goal > 0xFFFFFF) {
        // TODO: Support goal wraparound with 24bit timers
        lai_warn("Timer wraparound is unsupported for 24bit timers, TODO");
        return LAI_ERROR_UNSUPPORTED;
    }

    while (lai_read_pm_timer_value() < goal)
        ;

    return LAI_ERROR_NONE;
}

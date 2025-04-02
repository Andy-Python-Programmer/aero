/*
 * Lightweight AML Interpreter
 * Copyright (C) 2018-2023 The lai authors
 */

/* PCI IRQ Routing */
/* Every PCI device that is capable of generating an IRQ has an "interrupt pin"
   field in its configuration space. Contrary to what most people believe, this
   field is valid for both the PIC and the I/O APIC. The PCI local bus spec clearly
   says the "interrupt line" field everyone trusts are simply for BIOS or OS-
   -specific use. Therefore, nobody should assume it contains the real IRQ. Instead,
   the four PCI pins should be used: LNKA, LNKB, LNKC and LNKD. */

#include <lai/helpers/pci.h>
#include <lai/helpers/resource.h>

#include "../core/eval.h"
#include "../core/libc.h"

int lai_pci_route(acpi_resource_t *dest, uint16_t seg, uint8_t bus, uint8_t slot,
                  uint8_t function) {

    uint8_t pin = (uint8_t)laihost_pci_readb(seg, bus, slot, function, 0x3D);
    if (!pin || pin > 4)
        return 1;

    if (lai_pci_route_pin(dest, seg, bus, slot, function, pin))
        return 1;
    return 0;
}

lai_api_error_t lai_pci_route_pin(acpi_resource_t *dest, uint16_t seg, uint8_t bus, uint8_t slot,
                                  uint8_t function, uint8_t pin) {
    LAI_CLEANUP_STATE lai_state_t state;
    lai_init_state(&state);

    LAI_ENSURE(pin && pin <= 4);

    // PCI numbers pins from 1, but ACPI numbers them from 0. Hence we
    // subtract 1 to arrive at the correct pin number.
    pin--;

    // find the PCI bus in the namespace
    lai_nsnode_t *handle = lai_pci_find_bus(seg, bus, &state);
    if (!handle)
        return LAI_ERROR_NO_SUCH_NODE;

    // read the PCI routing table
    lai_nsnode_t *prt_handle = lai_resolve_path(handle, "_PRT");
    if (!prt_handle) {
        lai_warn("host bridge has no _PRT");
        return LAI_ERROR_NO_SUCH_NODE;
    }

    LAI_CLEANUP_VAR lai_variable_t prt = LAI_VAR_INITIALIZER;

    if (lai_eval(&prt, prt_handle, &state)) {
        lai_warn("failed to evaluate _PRT");
        return LAI_ERROR_EXECUTION_FAILURE;
    }

    struct lai_prt_iterator iter = LAI_PRT_ITERATOR_INITIALIZER(&prt);
    lai_api_error_t err;

    while (!(err = lai_pci_parse_prt(&iter))) {
        if (iter.slot == slot && (iter.function == function || iter.function == -1)
            && iter.pin == pin) {
            dest->type = ACPI_RESOURCE_IRQ;
            dest->base = iter.gsi;
            dest->irq_flags = (iter.level_triggered ? 0 : ACPI_SMALL_IRQ_EDGE_TRIGGERED)
                              | (iter.active_low ? ACPI_SMALL_IRQ_ACTIVE_LOW : 0);
            return LAI_ERROR_NONE;
        }
    }

    return err;
}

lai_api_error_t lai_pci_parse_prt(struct lai_prt_iterator *iter) {
    LAI_CLEANUP_VAR lai_variable_t prt_entry = LAI_VAR_INITIALIZER;
    LAI_CLEANUP_VAR lai_variable_t prt_entry_addr = LAI_VAR_INITIALIZER;
    LAI_CLEANUP_VAR lai_variable_t prt_entry_pin = LAI_VAR_INITIALIZER;
    LAI_CLEANUP_VAR lai_variable_t prt_entry_type = LAI_VAR_INITIALIZER;
    LAI_CLEANUP_VAR lai_variable_t prt_entry_index = LAI_VAR_INITIALIZER;

    if (lai_obj_get_pkg(iter->prt, iter->i, &prt_entry))
        return LAI_ERROR_UNEXPECTED_RESULT;

    iter->i++;

    if (lai_obj_get_pkg(&prt_entry, 0, &prt_entry_addr))
        return LAI_ERROR_UNEXPECTED_RESULT;
    if (lai_obj_get_pkg(&prt_entry, 1, &prt_entry_pin))
        return LAI_ERROR_UNEXPECTED_RESULT;
    if (lai_obj_get_pkg(&prt_entry, 2, &prt_entry_type))
        return LAI_ERROR_UNEXPECTED_RESULT;
    if (lai_obj_get_pkg(&prt_entry, 3, &prt_entry_index))
        return LAI_ERROR_UNEXPECTED_RESULT;

    uint64_t addr;
    if (lai_obj_get_integer(&prt_entry_addr, &addr))
        return LAI_ERROR_UNEXPECTED_RESULT;

    iter->slot = (addr >> 16) & 0xFFFF;
    iter->function = addr & 0xFFFF;

    if (iter->function == 0xFFFF)
        iter->function = -1;

    uint64_t pin;
    if (lai_obj_get_integer(&prt_entry_pin, &pin))
        return LAI_ERROR_UNEXPECTED_RESULT;

    iter->pin = pin;

    enum lai_object_type type = lai_obj_get_type(&prt_entry_type);
    if (type == LAI_TYPE_INTEGER) { // direct routing to GSI
        uint64_t gsi;
        if (lai_obj_get_integer(&prt_entry_index, &gsi))
            return LAI_ERROR_UNEXPECTED_RESULT;

        // TODO: Look up the GSI in the _CRS of the host bridge.
        iter->link = NULL;
        iter->resource_idx = 0;
        iter->level_triggered = 1;
        iter->active_low = 1;
        iter->gsi = gsi;
        return LAI_ERROR_NONE;
    } else if (type == LAI_TYPE_DEVICE) { // GSI obtained via a link dev
        lai_nsnode_t *link_handle;
        uint64_t res_index;
        if (lai_obj_get_handle(&prt_entry_type, &link_handle))
            return LAI_ERROR_UNEXPECTED_RESULT;
        if (lai_obj_get_integer(&prt_entry_index, &res_index))
            return LAI_ERROR_UNEXPECTED_RESULT;

        // Get _CRS of the link device.
        LAI_CLEANUP_STATE lai_state_t state;
        lai_init_state(&state);

        lai_nsnode_t *crs_handle = lai_resolve_path(link_handle, "_CRS");
        if (!crs_handle)
            return LAI_ERROR_UNEXPECTED_RESULT;

        LAI_CLEANUP_VAR lai_variable_t crs_buffer = LAI_VAR_INITIALIZER;
        int status = lai_eval(&crs_buffer, crs_handle, &state);
        if (status)
            return LAI_ERROR_EXECUTION_FAILURE;

        // Find the _CRS entry based on its index.
        struct lai_resource_view view = LAI_RESOURCE_VIEW_INITIALIZER(&crs_buffer);
        unsigned int current = 0;
        while (!lai_resource_iterate(&view)) {
            if (current == res_index) {
                enum lai_resource_type type = lai_resource_get_type(&view);
                if (type != LAI_RESOURCE_IRQ)
                    return LAI_ERROR_UNEXPECTED_RESULT;
                if (lai_resource_next_irq(&view))
                    return LAI_ERROR_UNEXPECTED_RESULT;
                iter->link = link_handle;
                iter->resource_idx = res_index;
                iter->gsi = view.gsi;
                iter->level_triggered = lai_resource_irq_is_level_triggered(&view);
                iter->active_low = lai_resource_irq_is_active_low(&view);
                return LAI_ERROR_NONE;
            }
            current++;
        }
        return LAI_ERROR_UNEXPECTED_RESULT;
    } else {
        lai_warn("PRT entry has unexpected type %ld", prt_entry_type.integer);
        return LAI_ERROR_TYPE_MISMATCH;
    }
}

lai_nsnode_t *lai_pci_find_device(lai_nsnode_t *bus, uint8_t slot, uint8_t function,
                                  lai_state_t *state) {
    LAI_ENSURE(bus);
    LAI_ENSURE(state);

    uint64_t device_adr = ((slot << 16) | function);

    struct lai_ns_child_iterator iter = LAI_NS_CHILD_ITERATOR_INITIALIZER(bus);
    lai_nsnode_t *node;
    while ((node = lai_ns_child_iterate(&iter))) {
        LAI_CLEANUP_VAR lai_variable_t adr = LAI_VAR_INITIALIZER;
        lai_nsnode_t *adr_handle = lai_resolve_path(node, "_ADR");
        if (adr_handle) {
            uint64_t adr_result;
            if (lai_eval(&adr, adr_handle, state)) {
                lai_warn("failed to evaluate _ADR");
                continue;
            }
            lai_obj_get_integer(&adr, &adr_result);

            if (adr_result == device_adr)
                return node;
        }
    }

    return NULL;
}

lai_nsnode_t *lai_pci_find_bus(uint16_t seg, uint8_t bus, lai_state_t *state) {
    LAI_CLEANUP_VAR lai_variable_t pci_pnp_id = LAI_VAR_INITIALIZER;
    LAI_CLEANUP_VAR lai_variable_t pcie_pnp_id = LAI_VAR_INITIALIZER;
    lai_eisaid(&pci_pnp_id, ACPI_PCI_ROOT_BUS_PNP_ID);
    lai_eisaid(&pcie_pnp_id, ACPI_PCIE_ROOT_BUS_PNP_ID);

    lai_nsnode_t *sb_handle = lai_resolve_path(NULL, "\\_SB_");
    LAI_ENSURE(sb_handle);
    struct lai_ns_child_iterator iter = LAI_NS_CHILD_ITERATOR_INITIALIZER(sb_handle);
    lai_nsnode_t *node;
    while ((node = lai_ns_child_iterate(&iter))) {
        if (lai_check_device_pnp_id(node, &pci_pnp_id, state)
            && lai_check_device_pnp_id(node, &pcie_pnp_id, state)) {
            continue;
        }

        LAI_CLEANUP_VAR lai_variable_t bus_number = LAI_VAR_INITIALIZER;
        uint64_t bbn_result = 0;
        lai_nsnode_t *bbn_handle = lai_resolve_path(node, "_BBN");
        if (bbn_handle) {
            if (lai_eval(&bus_number, bbn_handle, state)) {
                lai_warn("failed to evaluate _BBN");
                continue;
            }
            lai_obj_get_integer(&bus_number, &bbn_result);
        }

        LAI_CLEANUP_VAR lai_variable_t seg_number = LAI_VAR_INITIALIZER;
        uint64_t seg_result = 0;
        lai_nsnode_t *seg_handle = lai_resolve_path(node, "_SEG");
        if (seg_handle) {
            if (lai_eval(&seg_number, seg_handle, state)) {
                lai_warn("failed to evaluate _SEG");
                continue;
            }
            lai_obj_get_integer(&seg_number, &seg_result);
        }

        if (seg_result == seg && bbn_result == bus) {
            return node;
        }
    }

    return NULL;
}

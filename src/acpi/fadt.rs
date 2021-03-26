//! The FADT ACPI table contains information about fixed register blocks pertaining to power management.
//!
//! **Notes**: <https://wiki.osdev.org/FADT>

use core::ptr;

use super::{sdt::SDT, GenericAddressStructure};

pub const SIGNATURE: &str = "FACP";

#[repr(packed)]
#[derive(Clone, Copy, Debug)]
pub struct FADT {
    pub header: SDT,
    pub firmware_ctrl: u32,
    pub dsdt: u32,

    // Field used in ACPI 1.0; no longer in use, for compatibility only
    reserved: u8,

    pub preferred_power_managament: u8,
    pub sci_interrupt: u16,
    pub smi_command_port: u32,
    pub acpi_enable: u8,
    pub acpi_disable: u8,
    pub s4_bios_req: u8,
    pub pstate_control: u8,
    pub pm1a_event_block: u32,
    pub pm1b_event_block: u32,
    pub pm1a_control_block: u32,
    pub pm1b_control_block: u32,
    pub pm2_control_block: u32,
    pub pm_timer_block: u32,
    pub gpe0_block: u32,
    pub gpe1_block: u32,
    pub pm1_event_length: u8,
    pub pm1_control_length: u8,
    pub pm2_control_length: u8,
    pub pm_timer_length: u8,
    pub gpe0_ength: u8,
    pub gpe1_length: u8,
    pub gpe1_base: u8,
    pub c_state_control: u8,
    pub worst_c2_latency: u16,
    pub worst_c3_latency: u16,
    pub flush_size: u16,
    pub flush_stride: u16,
    pub duty_offset: u8,
    pub duty_width: u8,
    pub day_alarm: u8,
    pub month_alarm: u8,
    pub century: u8,

    // Reserved in ACPI 1.0; used since ACPI 2.0+
    pub boot_architecture_flags: u16,
    reserved2: u8,

    pub flags: u32,
    pub reset_register: GenericAddressStructure,

    pub reset_value: u8,
    reserved3: [u8; 3],

    // 64 bit pointers - Available on ACPI 2.0+
    pub x_firmware_control: u64,
    pub x_dsdt: u64,

    pub x_p_m1a_event_block: GenericAddressStructure,
    pub x_p_m1b_event_block: GenericAddressStructure,
    pub x_p_m1a_control_block: GenericAddressStructure,
    pub x_p_m1b_control_block: GenericAddressStructure,
    pub x_p_m2_control_block: GenericAddressStructure,
    pub x_p_m_timer_block: GenericAddressStructure,
    pub x_g_p_e0_block: GenericAddressStructure,
    pub x_g_p_e1_block: GenericAddressStructure,
}

impl FADT {
    pub fn new(sdt: Option<&'static SDT>) -> Self {
        let sdt = sdt.expect("FADT not found");

        unsafe { ptr::read((sdt as *const SDT) as *const Self) }
    }
}

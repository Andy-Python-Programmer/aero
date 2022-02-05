/*
 * Copyright (C) 2021-2022 The Aero Project Developers.
 *
 * This file is part of The Aero Project.
 *
 * Aero is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Aero is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Aero. If not, see <https://www.gnu.org/licenses/>.
 */

//! Thread Local Storage (TLS) are per-thread global variables. On 64-bit each CPU core's
//! `fs` GDT segment points to the thread local memory area where the thread local static's
//! live. TLS statics are simply accessed through an offset from `fs`.
//!
//! ## Notes
//! * <https://wiki.osdev.org/Thread_Local_Storage>
//! * <https://doc.rust-lang.org/std/thread/struct.LocalKey.html>

use core::alloc::Layout;

use alloc::alloc::alloc_zeroed;
use alloc::string::String;
use alloc::vec::Vec;

use super::gdt::*;

use crate::userland::scheduler;
use crate::utils::io;
use crate::utils::sync::Mutex;

use raw_cpuid::FeatureInfo;

type ProcFsCpuFeature = (&'static str, fn(&FeatureInfo) -> bool);

const CPU_FEATURES: &[ProcFsCpuFeature] = &[
    ("sse3", FeatureInfo::has_sse3),
    ("pclmulqdq", FeatureInfo::has_pclmulqdq),
    ("ds_area", FeatureInfo::has_ds_area),
    ("monitor_mwait", FeatureInfo::has_monitor_mwait),
    ("cpl", FeatureInfo::has_cpl),
    ("vmx", FeatureInfo::has_vmx),
    ("smx", FeatureInfo::has_smx),
    ("eist", FeatureInfo::has_eist),
    ("tm2", FeatureInfo::has_tm2),
    ("ssse3", FeatureInfo::has_ssse3),
    ("cnxtid", FeatureInfo::has_cnxtid),
    ("fma", FeatureInfo::has_fma),
    ("cmpxchg16b", FeatureInfo::has_cmpxchg16b),
    ("pdcm", FeatureInfo::has_pdcm),
    ("pcid", FeatureInfo::has_pcid),
    ("dca", FeatureInfo::has_dca),
    ("sse41", FeatureInfo::has_sse41),
    ("sse42", FeatureInfo::has_sse42),
    ("x2apic", FeatureInfo::has_x2apic),
    ("movbe", FeatureInfo::has_movbe),
    ("popcnt", FeatureInfo::has_popcnt),
    ("tsc_deadline", FeatureInfo::has_tsc_deadline),
    ("aesni", FeatureInfo::has_aesni),
    ("xsave", FeatureInfo::has_xsave),
    ("oxsave", FeatureInfo::has_oxsave),
    ("avx", FeatureInfo::has_avx),
    ("f16c", FeatureInfo::has_f16c),
    ("rdrand", FeatureInfo::has_rdrand),
    ("hypervisor", FeatureInfo::has_hypervisor),
    ("fpu", FeatureInfo::has_fpu),
    ("vme", FeatureInfo::has_vme),
    ("de", FeatureInfo::has_de),
    ("pse", FeatureInfo::has_pse),
    ("tsc", FeatureInfo::has_tsc),
    ("msr", FeatureInfo::has_msr),
    ("pae", FeatureInfo::has_pae),
    ("mce", FeatureInfo::has_mce),
    ("cmpxchg8b", FeatureInfo::has_cmpxchg8b),
    ("apic", FeatureInfo::has_apic),
    ("sysenter_sysexit", FeatureInfo::has_sysenter_sysexit),
    ("mtrr", FeatureInfo::has_mtrr),
    ("pge", FeatureInfo::has_pge),
    ("mca", FeatureInfo::has_mca),
    ("cmov", FeatureInfo::has_cmov),
    ("pat", FeatureInfo::has_pat),
    ("pse36", FeatureInfo::has_pse36),
    ("psn", FeatureInfo::has_psn),
    ("clflush", FeatureInfo::has_clflush),
    ("ds", FeatureInfo::has_ds),
    ("acpi", FeatureInfo::has_acpi),
    ("mmx", FeatureInfo::has_mmx),
    ("fxsave_fxstor", FeatureInfo::has_fxsave_fxstor),
    ("sse", FeatureInfo::has_sse),
    ("sse2", FeatureInfo::has_sse2),
    ("ss", FeatureInfo::has_ss),
    ("htt", FeatureInfo::has_htt),
    ("tm", FeatureInfo::has_tm),
    ("pbe", FeatureInfo::has_pbe),
];

static CPU_INFO: Mutex<Vec<CpuInfo>> = Mutex::new(Vec::new());

pub struct CpuInfo {
    pub cpuid: usize,

    pub fpu: bool,
    pub vendor: Option<String>,
    pub brand: Option<String>,
    pub features: Vec<&'static &'static str>,
}

pub struct PerCpuData {
    pub cpuid: usize,

    pub(super) gdt: &'static mut [GdtEntry],
}

/// SAFETY: The GS base should point to the kernel PCR.
pub fn get_cpuid() -> usize {
    get_percpu().cpuid
}

/// SAFETY: The GS base should point to the kernel PCR.
pub fn get_percpu() -> &'static mut PerCpuData {
    unsafe { (&mut *(io::rdmsr(io::IA32_GS_BASE) as *mut Kpcr)).cpu_local }
}

pub fn init(cpuid: usize) {
    let size = core::mem::size_of::<PerCpuData>();

    // NOTE: Inside kernel space, the GS base will always point to the CPU local data and when
    // jumping to userland `swapgs` is called making the GS base point to the userland TLS data.
    unsafe {
        let tss_layout = Layout::from_size_align_unchecked(
            core::mem::size_of::<Kpcr>(),
            core::mem::align_of::<Kpcr>(),
        );

        let tss_ptr = alloc_zeroed(tss_layout) as *mut Tss;
        io::wrmsr(io::IA32_GS_BASE, tss_ptr as u64);

        let tls_layout = Layout::from_size_align_unchecked(size, 8);
        let tls_raw_ptr = alloc_zeroed(tls_layout);

        crate::arch::gdt::get_kpcr().cpu_local = &mut *(tls_raw_ptr as *mut PerCpuData);
    }

    get_percpu().cpuid = cpuid;

    let cpuid = raw_cpuid::CpuId::new();

    let features = cpuid
        .get_feature_info()
        .map(|cpu_features| {
            CPU_FEATURES
                .iter()
                .filter(|(_, check_fn)| (check_fn)(&cpu_features))
                .map(|(name, _)| name)
                .collect::<Vec<_>>()
        })
        .unwrap_or(Vec::new());

    CPU_INFO.lock().push(CpuInfo {
        cpuid: 0,

        fpu: cpuid
            .get_feature_info()
            .map(|e| e.has_fpu())
            .unwrap_or(false),

        features,
        vendor: cpuid.get_vendor_info().map(|e| String::from(e.as_str())),
        brand: cpuid
            .get_processor_brand_string()
            .map(|e| String::from(e.as_str())),
    })
}

pub fn for_cpu_info_cached<F>(mut f: F)
where
    F: FnMut(&CpuInfo),
{
    let lock = CPU_INFO.lock();

    for info in lock.iter() {
        f(&info);
    }
}

#[no_mangle]
extern "C" fn restore_user_tls() {
    unsafe {
        let base = scheduler::get_scheduler()
            .current_task()
            .arch_task_mut()
            .get_fs_base();

        io::wrmsr(io::IA32_FS_BASE, base.as_u64());
    }
}

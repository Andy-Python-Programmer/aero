#![no_std]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]

use core::{ops, slice};

use aero_gfx::FrameBuffer;
use x86_64::VirtAddr;

/// Represents the different types of memory.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[non_exhaustive]
#[repr(C)]
pub enum MemoryRegionType {
    /// Unused conventional memory, can be used by the kernel.
    Usable,
    /// Memory mappings created by the bootloader, including the kernel and boot info mappings.
    ///
    /// This memory should **not** be used by the kernel.
    Bootloader,
    UnknownUefi(u32),
    UnknownBios(u32),
}

#[derive(Debug)]
#[repr(C)]
pub struct BootInfo {
    pub rsdp_address: u64,
    pub physical_memory_offset: u64,
    pub framebuffer: FrameBuffer,
    pub memory_regions: MemoryRegions,
    pub stack_top: VirtAddr,
}

#[derive(Debug)]
#[repr(C)]
pub struct MemoryRegions {
    pub(crate) ptr: *mut MemoryRegion,
    pub(crate) len: usize,
}

impl ops::Deref for MemoryRegions {
    type Target = [MemoryRegion];

    fn deref(&self) -> &Self::Target {
        unsafe { slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl ops::DerefMut for MemoryRegions {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

impl From<&'static mut [MemoryRegion]> for MemoryRegions {
    fn from(regions: &'static mut [MemoryRegion]) -> Self {
        MemoryRegions {
            ptr: regions.as_mut_ptr(),
            len: regions.len(),
        }
    }
}

impl From<MemoryRegions> for &'static mut [MemoryRegion] {
    fn from(regions: MemoryRegions) -> &'static mut [MemoryRegion] {
        unsafe { slice::from_raw_parts_mut(regions.ptr, regions.len) }
    }
}

/// Represent a physical memory region.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
pub struct MemoryRegion {
    /// The physical start address of the region.
    pub start: u64,
    /// The physical end address (exclusive) of the region.
    pub end: u64,
    /// The memory type of the memory region.
    pub kind: MemoryRegionType,
}

pub fn test_runner(tests: &[&dyn Fn()]) {
    for test in tests {
        test();
    }
}

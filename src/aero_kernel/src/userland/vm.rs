/*
 * Copyright (C) 2021 The Aero Project Developers.
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

use aero_syscall::{MMapFlags, MMapProt};

use alloc::collections::linked_list::CursorMut;
use alloc::collections::LinkedList;

use crate::mem::paging::*;
use crate::mem::AddressSpace;

use spin::Mutex;
use xmas_elf::ElfFile;

use super::USERLAND_SHELL;

impl From<MMapProt> for PageTableFlags {
    fn from(e: MMapProt) -> Self {
        let mut res = PageTableFlags::empty();

        if !e.contains(MMapProt::PROT_EXEC) {
            res.insert(PageTableFlags::NO_EXECUTE);
        }

        if e.contains(MMapProt::PROT_WRITE) {
            res.insert(PageTableFlags::WRITABLE);
        }

        res
    }
}

#[derive(Clone)]
pub struct MMapFile {
    offset: usize,
    // FIXME(Andy-Python-Programmer): Use an actual filesystem file instead of
    // directly storing the bytes.
    file: &'static [u8],
    size: usize,
}

impl MMapFile {
    #[inline]
    fn new(file: &'static [u8], offset: usize, size: usize) -> Self {
        Self { file, offset, size }
    }
}

#[derive(Clone)]
struct Mapping {
    protocol: MMapProt,
    flags: MMapFlags,

    start_addr: VirtAddr,
    end_addr: VirtAddr,

    file: Option<MMapFile>,
}

impl Mapping {
    /// Handler routine for private anonymous pages. Since its an annonymous page is not
    /// backed by a file, we have to alloctate a frame and map it at the faulted address.
    fn handle_pf_private_anon(
        &mut self,
        offset_table: &mut OffsetPageTable,
        reason: PageFaultErrorCode,
        address: VirtAddr,
    ) -> bool {
        let addr_aligned = address.align_down(Size4KiB::SIZE);

        if !reason.contains(PageFaultErrorCode::PROTECTION_VIOLATION) {
            log::trace!(
                "    - private R: {:#x}..{:#x}",
                addr_aligned,
                addr_aligned + Size4KiB::SIZE
            );

            let frame = unsafe { FRAME_ALLOCATOR.allocate_frame() }
                .expect("Failed to allocate frame for userland mapping");

            unsafe {
                offset_table.map_to(
                    Page::containing_address(addr_aligned),
                    frame,
                    PageTableFlags::USER_ACCESSIBLE
                        | PageTableFlags::PRESENT
                        | self.protocol.into(),
                    &mut FRAME_ALLOCATOR,
                )
            }
            .expect("Failed to identity map userspace private mapping")
            .flush();

            true
        } else if reason.contains(PageFaultErrorCode::CAUSED_BY_WRITE) {
            log::trace!(
                "    - private COW: {:#x}..{:#x}",
                addr_aligned,
                addr_aligned + Size4KiB::SIZE
            );

            self.handle_cow(offset_table, addr_aligned)
        } else {
            log::error!("    - present page read failed");

            false
        }
    }

    /// Handler routine for pages backed by a file. This function will allocate a frame and
    /// read a page-sized amount from the disk into the allocated frame. Then it maps
    /// the allocated frame at the faulted address.
    fn handle_pf_private_file(
        &mut self,
        offset_table: &mut OffsetPageTable,
        reason: PageFaultErrorCode,
        address: VirtAddr,
    ) -> bool {
        if let Some(mmap_file) = self.file.as_mut() {
            let offset = align_down(
                (address - self.start_addr) + mmap_file.offset as u64,
                Size4KiB::SIZE,
            );

            let address = address.align_down(Size4KiB::SIZE);
            let size = core::cmp::min(
                Size4KiB::SIZE,
                mmap_file.size as u64 - (address - self.start_addr),
            );

            if !reason.contains(PageFaultErrorCode::CAUSED_BY_WRITE)
                && !reason.contains(PageFaultErrorCode::PROTECTION_VIOLATION)
            {
                // We are writing to private file mapping so copy the content of the page.
                log::trace!(
                    "    - private file R: {:?}..{:?} (offset={:#x})",
                    address,
                    address + size,
                    offset
                );

                let frame = unsafe { FRAME_ALLOCATOR.allocate_frame() }
                    .expect("failed to allocate frame for a private file read");

                unsafe {
                    (mmap_file.file.as_ptr()).offset(offset as isize).copy_to(
                        (crate::PHYSICAL_MEMORY_OFFSET + frame.start_address().as_u64())
                            .as_mut_ptr(),
                        size as usize,
                    );
                }

                let mut flags = PageTableFlags::PRESENT
                    | PageTableFlags::USER_ACCESSIBLE
                    | self.protocol.into();

                // We want to remove the writable flag since, we want to share the page table
                // entry with other processes or threads until it tries to write to the same page
                // and the mapping is marked as writable, in that case we will copy the page table
                // entry.
                flags.remove(PageTableFlags::WRITABLE);

                unsafe {
                    offset_table.map_to(
                        Page::containing_address(address),
                        frame,
                        flags,
                        &mut FRAME_ALLOCATOR,
                    )
                }
                .expect("failed to map allocated frame for private file read")
                .flush();
            } else if reason.contains(PageFaultErrorCode::CAUSED_BY_WRITE)
                && !reason.contains(PageFaultErrorCode::PROTECTION_VIOLATION)
            {
                log::trace!("    - private file C: {:?}", address);
                unimplemented!()
            } else if reason.contains(PageFaultErrorCode::PROTECTION_VIOLATION)
                && reason.contains(PageFaultErrorCode::CAUSED_BY_WRITE)
            {
                log::trace!("    - private file COW: {:?}", address);
                unimplemented!()
            }

            true
        } else {
            false
        }
    }

    /// Handler routine for a COW (Copy-On-Write) pages. A COW page is shared between multiple processes
    /// until a write occurs after which a private copy is made for the writing process. A COW page
    /// is recognised because the VMA for the region is marked writable even though the individual page
    /// table entry is not.
    fn handle_cow(&mut self, offset_table: &mut OffsetPageTable, address: VirtAddr) -> bool {
        if offset_table.translate_addr(address).is_some() {
            // This page is not shared with anyone, so just make it writable.
            let page: Page = Page::containing_address(address);

            unsafe {
                offset_table
                    .update_flags(
                        page,
                        PageTableFlags::USER_ACCESSIBLE
                            | PageTableFlags::PRESENT
                            | self.protocol.into(),
                    )
                    .expect("Failed to update flags")
                    .flush();
            }

            true
        } else {
            false
        }
    }
}

struct VmProtected {
    mappings: LinkedList<Mapping>,
}

impl VmProtected {
    fn new() -> Self {
        Self {
            mappings: LinkedList::new(),
        }
    }

    fn handle_page_fault(
        &mut self,
        reason: PageFaultErrorCode,
        accessed_address: VirtAddr,
    ) -> bool {
        if let Some(map) = self
            .mappings
            .iter_mut()
            .find(|e| accessed_address >= e.start_addr && accessed_address < e.end_addr)
        {
            log::trace!("mapping {:?} on demand", accessed_address);

            if map.protocol.is_empty() {
                return false;
            }

            if reason.contains(PageFaultErrorCode::CAUSED_BY_WRITE)
                && !map.protocol.contains(MMapProt::PROT_WRITE)
            {
                return false;
            }

            if reason.contains(PageFaultErrorCode::INSTRUCTION_FETCH)
                && !map.protocol.contains(MMapProt::PROT_EXEC)
            {
                return false;
            }

            let is_private = map.flags.contains(MMapFlags::MAP_PRIVATE);
            let is_annon = map.flags.contains(MMapFlags::MAP_ANONYOMUS);

            let mut address_space = AddressSpace::this();
            let mut offset_table = address_space.offset_page_table();

            match (is_private, is_annon) {
                (true, true) => {
                    map.handle_pf_private_anon(&mut offset_table, reason, accessed_address)
                }

                (true, false) => {
                    map.handle_pf_private_file(&mut offset_table, reason, accessed_address)
                }

                (false, true) => unreachable!("shared and anonymous mapping"),
                (false, false) => unimplemented!(),
            }
        } else {
            log::trace!("mapping not found for address: {:#x}", accessed_address);

            // Else the mapping does not exist, so return false.
            false
        }
    }

    fn find_fixed_mapping(
        &mut self,
        address: VirtAddr,
        size: usize,
    ) -> Option<(VirtAddr, CursorMut<Mapping>)> {
        let mut cursor = self.mappings.cursor_front_mut();

        while let Some(map) = cursor.current() {
            if map.start_addr <= address && map.end_addr > address {
                return None;
            } else if map.start_addr < address {
                cursor.move_next();
            } else {
                if address + size > map.start_addr {
                    return None;
                } else {
                    break;
                }
            }
        }

        Some((address, cursor))
    }

    fn mmap(
        &mut self,
        address: VirtAddr,
        size: usize,
        protocol: MMapProt,
        flags: MMapFlags,
        offset: usize,
        file: Option<&'static [u8]>,
    ) -> Option<VirtAddr> {
        // Offset is required to be a multiple of page size.
        if (offset as u64 & Size4KiB::SIZE - 1) != 0 {
            return None;
        }

        // The provided size should be non-zero.
        if size == 0 {
            return None;
        }

        let size_aligned = align_up(size as _, Size4KiB::SIZE);

        if flags.contains(MMapFlags::MAP_FIXED) {
            self.find_fixed_mapping(address, size_aligned as _)
                .and_then(|(addr, mut cursor)| {
                    // Merge same mappings instead of creating a new one.
                    if let Some(prev) = cursor.peek_prev() {
                        if prev.end_addr == addr
                            && prev.flags == flags
                            && prev.protocol == protocol
                            && prev.file.is_none()
                        {
                            prev.end_addr = addr + size_aligned;

                            return Some(addr);
                        }
                    }

                    cursor.insert_before(Mapping {
                        protocol,
                        flags,

                        start_addr: addr,
                        end_addr: addr + size_aligned,

                        file: file.map(|f| MMapFile::new(f, offset, size)),
                    });

                    Some(addr)
                })
        } else {
            None
        }
    }

    fn load_bin(&mut self, bin: &ElfFile) {
        log::debug!("entry point: {:#x}", bin.header.pt2.entry_point());
        log::debug!("entry point type: {:?}", bin.header.pt2.type_().as_type());

        for header in bin.program_iter() {
            xmas_elf::program::sanity_check(header, bin).expect("Failed header sanity check");

            let header_type = header
                .get_type()
                .expect("Failed to get program header type");

            let header_flags = header.flags();

            if header_type == xmas_elf::program::Type::Load {
                let virtual_start = VirtAddr::new(header.virtual_addr()).align_down(Size4KiB::SIZE);

                let virtual_end = VirtAddr::new(header.virtual_addr() + header.mem_size())
                    .align_up(Size4KiB::SIZE);

                let virtual_fend = VirtAddr::new(header.virtual_addr() + header.file_size());

                let len = virtual_fend - virtual_start;
                let file_offset = align_down(header.offset(), Size4KiB::SIZE);

                let mut prot = MMapProt::empty();

                if header_flags.is_read() {
                    prot.insert(MMapProt::PROT_READ);
                }

                if header_flags.is_write() {
                    prot.insert(MMapProt::PROT_WRITE);
                }

                if header_flags.is_execute() {
                    prot.insert(MMapProt::PROT_EXEC);
                }

                let virtual_fend = self
                    .mmap(
                        virtual_start,
                        len as usize,
                        prot,
                        MMapFlags::MAP_PRIVATE | MMapFlags::MAP_FIXED,
                        file_offset as usize,
                        Some(USERLAND_SHELL),
                    )
                    .expect("Failed to memory map ELF header")
                    + align_up(len, Size4KiB::SIZE);

                if virtual_fend < virtual_end {
                    let len = virtual_end - virtual_fend;

                    self.mmap(
                        virtual_fend,
                        len as usize,
                        prot,
                        MMapFlags::MAP_PRIVATE | MMapFlags::MAP_ANONYOMUS | MMapFlags::MAP_FIXED,
                        0x00,
                        None,
                    );
                }
            } else if header_type == xmas_elf::program::Type::Tls {
            } else if header_type == xmas_elf::program::Type::Interp {
            }
        }
    }
}

pub struct Vm {
    inner: Mutex<VmProtected>,
}

impl Vm {
    /// Creates a new instance of VM.
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            inner: Mutex::new(VmProtected::new()),
        }
    }

    #[inline]
    pub fn mmap(
        &self,
        address: VirtAddr,
        size: usize,
        protocol: MMapProt,
        flags: MMapFlags,
    ) -> Option<VirtAddr> {
        self.inner
            .lock()
            .mmap(address, size, protocol, flags, 0x00, None)
    }

    /// Mapping the provided `bin` file into the VM.
    #[inline]
    pub(super) fn load_bin(&self, bin: &ElfFile) {
        self.inner.lock().load_bin(bin)
    }

    /// Clears all of the mappings in the VM.
    #[inline]
    pub(super) fn clear(&self) {
        self.inner.lock().mappings.clear()
    }

    /// This function is responsible for handling page faults occured in
    /// user mode. It determines the address, the reason of the page fault
    /// and then passes it off to one of the appropriate page fault handlers.
    #[inline]
    pub(crate) fn handle_page_fault(
        &self,
        reason: PageFaultErrorCode,
        accessed_address: VirtAddr,
    ) -> bool {
        self.inner
            .lock()
            .handle_page_fault(reason, accessed_address)
    }

    pub(crate) fn log(&self) {
        let this = self.inner.lock();

        for mmap in &this.mappings {
            if let Some(file) = mmap.file.as_ref() {
                log::debug!(
                    "{:?}..{:?} => {:?}, {:?} (offset={:#x}, size={:#x})",
                    mmap.start_addr,
                    mmap.end_addr,
                    mmap.protocol,
                    mmap.flags,
                    file.offset,
                    file.size,
                );
            } else {
                log::debug!(
                    "{:?}..{:?} => {:?}, {:?}",
                    mmap.start_addr,
                    mmap.end_addr,
                    mmap.protocol,
                    mmap.flags,
                );
            }
        }
    }
}

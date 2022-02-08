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

use aero_syscall::{MMapFlags, MMapProt};

use alloc::boxed::Box;
use alloc::collections::linked_list::CursorMut;
use alloc::collections::LinkedList;

use xmas_elf::header::*;
use xmas_elf::program::*;
use xmas_elf::*;

use crate::arch::task::userland_last_address;
use crate::fs;
use crate::fs::cache::DirCacheItem;
use crate::fs::FileSystemError;
use crate::mem;
use crate::mem::paging::*;
use crate::mem::AddressSpace;

use crate::utils::sync::Mutex;

const ELF_HEADER_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];

const ELF_PT1_SIZE: usize = core::mem::size_of::<HeaderPt1>();
const ELF_PT2_64_SIZE: usize = core::mem::size_of::<HeaderPt2_<P64>>();

#[derive(Debug)]
pub enum ElfParseError {
    /// Unexpected file system error occured when reading the file.
    ReadError(FileSystemError),
    /// The PT1 header has an invalid magic number.
    InvalidMagic,
    /// The ELF header contains an invalid class.
    InvalidClass,
    /// The provided program header index is invalid.
    InvalidProgramHeaderIndex,
}

// TODO: Remove the Box::leak() calls
fn parse_elf_header<'header>(file: DirCacheItem) -> Result<Header<'header>, ElfParseError> {
    // 1. Read the ELF PT1 header:
    let mut pt1_hdr_slice = Box::leak(mem::alloc_boxed_buffer::<u8>(ELF_PT1_SIZE));

    file.inode()
        .read_at(0, &mut pt1_hdr_slice)
        .map_err(|err| ElfParseError::ReadError(err))?;

    let pt1_header: &'header _ = unsafe { &*(pt1_hdr_slice.as_ptr() as *const HeaderPt1) };

    // 2. Ensure that the header has the correct magic number:
    if pt1_header.magic != ELF_HEADER_MAGIC {
        return Err(ElfParseError::InvalidMagic);
    }

    let pt2_header = match pt1_header.class() {
        // 3. Read the 64-bit PT2 header:
        Class::SixtyFour => {
            let mut pt2_hdr_slice = Box::leak(mem::alloc_boxed_buffer::<u8>(ELF_PT2_64_SIZE));

            file.inode()
                .read_at(ELF_PT1_SIZE, &mut pt2_hdr_slice)
                .map_err(|err| ElfParseError::ReadError(err))?;

            let pt2_header_ptr = pt2_hdr_slice.as_ptr();
            let pt2_header: &'header _ = unsafe { &*(pt2_header_ptr as *const HeaderPt2_<P64>) };

            Ok(HeaderPt2::Header64(pt2_header))
        }

        // 3. Read the 32-bit PT2 header:
        Class::ThirtyTwo => {
            unimplemented!("parse_elf_header: 32-bit executables are not implemented")
        }

        // SAFTEY: ensure the PT1 header has a valid class.
        Class::None | Class::Other(_) => Err(ElfParseError::InvalidClass),
    }?;

    Ok(Header {
        pt1: pt1_header,
        pt2: pt2_header,
    })
}

// TODO: Remove the Box::leak() calls
fn parse_program_header<'pheader>(
    file: DirCacheItem,
    header: Header<'pheader>,
    index: u16,
) -> Result<ProgramHeader<'pheader>, ElfParseError> {
    let pt2 = &header.pt2;

    // SAFTEY: ensure that the provided program header index is valid.
    if !(index < pt2.ph_count() && pt2.ph_offset() > 0 && pt2.ph_entry_size() > 0) {
        return Err(ElfParseError::InvalidProgramHeaderIndex);
    }

    // 1. Calculate the start offset and size of the program header:
    let start = pt2.ph_offset() as usize + index as usize * pt2.ph_entry_size() as usize;
    let size = pt2.ph_entry_size() as usize;

    // 2. Read the 64-bit program header:
    let mut phdr_buffer = Box::leak(mem::alloc_boxed_buffer::<u8>(size));

    file.inode()
        .read_at(start, &mut phdr_buffer)
        .map_err(|err| ElfParseError::ReadError(err))?;

    let phdr_ptr = phdr_buffer.as_ptr();

    match header.pt1.class() {
        // 3. Cast and return the 64-bit program header:
        Class::SixtyFour => {
            let phdr: &'pheader _ = unsafe { &*(phdr_ptr as *const ProgramHeader64) };
            Ok(ProgramHeader::Ph64(phdr))
        }

        // 3. Cast and return the 32-bit program header:
        Class::ThirtyTwo => {
            let phdr: &'pheader _ = unsafe { &*(phdr_ptr as *const ProgramHeader32) };
            Ok(ProgramHeader::Ph32(phdr))
        }

        // SAFTEY: ensure the PT1 header has a valid class.
        Class::None | Class::Other(_) => Err(ElfParseError::InvalidClass),
    }
}

struct ProgramHeaderIter<'this> {
    file: DirCacheItem,
    header: Header<'this>,
    next_index: usize,
}

impl<'this> ProgramHeaderIter<'this> {
    fn new(header: Header<'this>, file: DirCacheItem) -> Self {
        Self {
            file,
            header,

            next_index: 0,
        }
    }
}

impl<'this> Iterator for ProgramHeaderIter<'this> {
    type Item = ProgramHeader<'this>;

    fn next(&mut self) -> Option<Self::Item> {
        let count = self.header.pt2.ph_count() as usize;

        // We have reached at the end of the program header array.
        if self.next_index >= count {
            return None;
        }

        // Parse and return the program header.
        let result =
            parse_program_header(self.file.clone(), self.header, self.next_index as u16).ok();

        // Increment the next index.
        self.next_index += 1;
        result
    }
}

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

enum UnmapResult {
    None,
    Parital(Mapping),
    Full,
    Start,
    End,
}

pub struct LoadedBinary<'header> {
    pub header: Header<'header>,

    pub entry_point: VirtAddr,
    pub base_addr: VirtAddr,
}

#[derive(Clone)]
pub struct MMapFile {
    offset: usize,
    file: DirCacheItem,
    size: usize,
}

impl MMapFile {
    #[inline]
    fn new(file: DirCacheItem, offset: usize, size: usize) -> Self {
        Self { file, offset, size }
    }
}

#[derive(Clone)]
struct Mapping {
    protection: MMapProt,
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

            let frame: PhysFrame =
                PhysFrame::containing_address(pmm_alloc(BuddyOrdering::Size4KiB));

            unsafe {
                offset_table.map_to(
                    Page::containing_address(addr_aligned),
                    frame,
                    PageTableFlags::USER_ACCESSIBLE
                        | PageTableFlags::PRESENT
                        | self.protection.into(),
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

            self.handle_cow(offset_table, addr_aligned, false)
        } else {
            log::error!("    - present page read failed");
            false
        }
    }

    /// Handler routine for pages backed by a file. This function will allocate a frame and
    /// read a page-sized amount from the disk into the allocated frame. Then it maps
    /// the allocated frame at the faulted address.
    fn handle_pf_file(
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

            if !reason.contains(PageFaultErrorCode::PROTECTION_VIOLATION) {
                // We are writing to private file mapping so copy the content of the page.
                log::trace!(
                    "    - private file R: {:?}..{:?} (offset={:#x})",
                    address,
                    address + size,
                    offset
                );

                let phys = mmap_file
                    .file
                    .inode()
                    .mmap(offset as usize, self.flags)
                    .expect("handle_pf_file: file does not support mmap");

                let frame: PhysFrame<Size4KiB> = PhysFrame::containing_address(phys);

                unsafe {
                    offset_table.map_to(
                        Page::containing_address(address),
                        frame,
                        PageTableFlags::PRESENT
                            | PageTableFlags::USER_ACCESSIBLE
                            | self.protection.into(),
                        &mut FRAME_ALLOCATOR,
                    )
                }
                .expect("failed to map allocated frame for private file read")
                .flush();

                true
            } else {
                log::error!("    - present page read failed");
                false
            }
        } else {
            false
        }
    }

    /// Handler routine for a COW (Copy-On-Write) pages. A COW page is shared between multiple processes
    /// until a write occurs after which a private copy is made for the writing process. A COW page
    /// is recognised because the VMA for the region is marked writable even though the individual page
    /// table entry is not.
    fn handle_cow(
        &mut self,
        offset_table: &mut OffsetPageTable,
        address: VirtAddr,
        copy: bool,
    ) -> bool {
        if let TranslateResult::Mapped { frame, .. } = offset_table.translate(address) {
            let addr = frame.start_address();
            let page: Page<Size4KiB> = Page::containing_address(address);

            if let Some(vm_frame) = addr.as_vm_frame() {
                if vm_frame.ref_count() > 1 || copy {
                    // This page is used by more then one process, so make it a private copy.
                    log::trace!("    - making {:?} into a private copy", page);

                    let frame = pmm_alloc(BuddyOrdering::Size4KiB);

                    unsafe {
                        address.as_ptr::<u8>().copy_to(
                            (crate::PHYSICAL_MEMORY_OFFSET + frame.as_u64()).as_mut_ptr(),
                            Size4KiB::SIZE as _,
                        );
                    }

                    offset_table.unmap(page).expect("unmap faild").1.flush();
                    let frame = PhysFrame::containing_address(frame);

                    unsafe {
                        offset_table.map_to(
                            page,
                            frame,
                            PageTableFlags::PRESENT
                                | PageTableFlags::USER_ACCESSIBLE
                                | self.protection.into(),
                            &mut FRAME_ALLOCATOR,
                        )
                    }
                    .expect("page mapping failed")
                    .flush();
                } else {
                    // This page is used by only one process, so make it writable.
                    log::trace!("    - making {:?} writable", page);

                    unsafe {
                        offset_table.update_flags(
                            page,
                            PageTableFlags::PRESENT
                                | PageTableFlags::USER_ACCESSIBLE
                                | self.protection.into(),
                        )
                    }
                    .expect("failed to update page table flags")
                    .flush();
                }

                true
            } else {
                false
            }
        } else {
            false
        }
    }

    fn unmap(
        &mut self,
        offset_table: &mut OffsetPageTable,
        start: VirtAddr,
        end: VirtAddr,
    ) -> Result<UnmapResult, UnmapError> {
        let mut unmap_range_inner = |range| -> Result<(), UnmapError> {
            match offset_table.unmap_range(range, Size4KiB::SIZE) {
                Ok(_) => Ok(()),

                // its fine since technically we are not actually allocating the range
                // and they are just allocated on faults. So there might be a chance where we
                // try to unmap a region that is mapped but not actually allocated.
                Err(UnmapError::PageNotMapped) => Ok(()),
                Err(err) => return Err(err),
            }
        };

        if end <= self.start_addr || start >= self.end_addr {
            Ok(UnmapResult::None)
        } else if start > self.start_addr && end < self.end_addr {
            // The address we want to unmap is in the middle of the region. So we
            // will need to split the mapping and update the end address accordingly.
            unmap_range_inner(start..end)?;

            let new_file = self.file.as_ref().map(|file| {
                let offset = file.offset + (end - self.start_addr) as usize;
                let size = file.size - (offset - file.offset);

                MMapFile::new(file.file.clone(), offset, size)
            });

            let new_mapping = Mapping {
                protection: self.protection.clone(),
                flags: self.flags.clone(),
                start_addr: end,
                end_addr: end + (self.end_addr - end),
                file: new_file,
            };

            self.end_addr = end;

            Ok(UnmapResult::Parital(new_mapping))
        } else if start <= self.start_addr && end >= self.end_addr {
            // We are unmapping the whole region.
            unmap_range_inner(self.start_addr..self.end_addr)?;
            Ok(UnmapResult::Full)
        } else if start <= self.start_addr && end < self.end_addr {
            unmap_range_inner(self.start_addr..end)?;

            // Update the start address of the mapping since we have unmapped the
            // first chunk of the mapping.
            let offset = end - self.start_addr;

            if let Some(file) = self.file.as_mut() {
                file.offset += offset as usize;
            }

            self.start_addr = end;

            Ok(UnmapResult::Start)
        } else {
            unmap_range_inner(start..self.end_addr)?;

            // Update the end address of the mapping since we have unmapped the
            // last chunk of the mapping.
            self.end_addr = end;
            Ok(UnmapResult::End)
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

            if map.protection.is_empty() {
                return false;
            }

            if reason.contains(PageFaultErrorCode::CAUSED_BY_WRITE)
                && !map.protection.contains(MMapProt::PROT_WRITE)
            {
                return false;
            }

            if reason.contains(PageFaultErrorCode::INSTRUCTION_FETCH)
                && !map.protection.contains(MMapProt::PROT_EXEC)
            {
                return false;
            }

            let is_private = map.flags.contains(MMapFlags::MAP_PRIVATE);
            let is_annon = map.flags.contains(MMapFlags::MAP_ANONYOMUS);

            let mut address_space = AddressSpace::this();
            let mut offset_table = address_space.offset_page_table();

            let result = match (is_private, is_annon) {
                (true, true) => {
                    map.handle_pf_private_anon(&mut offset_table, reason, accessed_address)
                }

                (true, false) | (false, false) => {
                    map.handle_pf_file(&mut offset_table, reason, accessed_address)
                }

                (false, true) => unreachable!("shared and anonymous mapping"),
            };

            result
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

    fn find_any_above(
        &mut self,
        address: VirtAddr,
        size: usize,
    ) -> Option<(VirtAddr, CursorMut<Mapping>)> {
        if self.mappings.is_empty() {
            return Some((address, self.mappings.cursor_front_mut()));
        }

        let mut cursor = self.mappings.cursor_front_mut();

        // Search the mappings starting at the current cursor position for a big
        // enough hole for where the address is above the provided `address`. A hole is
        // big enough if it can hold the requested `size`. We use the first fit strategy,
        // so it breaks as soon as a big enough hole is found.
        while let Some(map) = cursor.current() {
            let map_start = map.start_addr;

            if map.start_addr < address {
                cursor.move_next();
            } else {
                if let Some(pmap) = cursor.peek_prev() {
                    let start = core::cmp::max(address, pmap.end_addr);
                    let hole = map_start.as_u64() - start.as_u64();

                    if hole as usize >= size {
                        return Some((start, cursor));
                    } else {
                        // The hole is too small
                        cursor.move_next();
                    }
                } else {
                    let hole = map_start.as_u64() - address.as_u64();

                    return if hole as usize >= size {
                        Some((address, cursor))
                    } else {
                        // The hole is too small
                        None
                    };
                }
            }
        }

        None
    }

    fn mmap(
        &mut self,
        address: VirtAddr,
        size: usize,
        protection: MMapProt,
        flags: MMapFlags,
        offset: usize,
        file: Option<DirCacheItem>,
    ) -> Option<VirtAddr> {
        // Offset is required to be a multiple of page size.
        if (offset as u64 & Size4KiB::SIZE - 1) != 0 {
            return None;
        }

        // The provided size should be non-zero.
        if size == 0 {
            return None;
        }

        if file.is_some() {
            // SAFTEY: We cannot mmap a file with the anonymous flag.
            if flags.contains(MMapFlags::MAP_ANONYOMUS) {
                return None;
            }
        } else {
            // SAFTEY: Mappings not backed by a file must be anonymous.
            if !flags.contains(MMapFlags::MAP_ANONYOMUS) {
                return None;
            }

            // SAFTEY: We cannot have a shared and an anonymous mapping.
            if flags.contains(MMapFlags::MAP_SHARED) {
                return None;
            }
        }

        let size_aligned = align_up(size as _, Size4KiB::SIZE);

        if address == VirtAddr::zero() {
            // We need to find a free mapping above 0x7000_0000_0000.
            self.find_any_above(VirtAddr::new(0x7000_0000_0000), size_aligned as _)
        } else {
            if flags.contains(MMapFlags::MAP_FIXED) {
                // SAFTEY: The provided address should be page aligned.
                if !address.is_aligned(Size4KiB::SIZE) {
                    return None;
                }

                // SAFTEY: The provided (address + size) should be less then
                // the userland max address.
                if (address + size_aligned) > userland_last_address() {
                    return None;
                }

                self.munmap(address, size_aligned as usize); // Unmap any existing mappings.
                self.find_fixed_mapping(address, size_aligned as _)
            } else {
                self.find_any_above(address, size)
            }
        }
        .and_then(|(addr, mut cursor)| {
            // Merge same mappings instead of creating a new one.
            if let Some(prev) = cursor.peek_prev() {
                if prev.end_addr == addr
                    && prev.flags == flags
                    && prev.protection == protection
                    && prev.file.is_none()
                {
                    prev.end_addr = addr + size_aligned;

                    return Some(addr);
                }
            }

            cursor.insert_before(Mapping {
                protection,
                flags,

                start_addr: addr,
                end_addr: addr + size_aligned,

                file: file.map(|f| MMapFile::new(f, offset, size)),
            });

            Some(addr)
        })
    }

    fn clear(&mut self) {
        let mut cursor = self.mappings.cursor_front_mut();

        let mut address_space = AddressSpace::this();
        let mut offset_table = address_space.offset_page_table();

        while let Some(map) = cursor.current() {
            // now this should automatically free the physical page backed by this mapping
            // if the reference count of that frame is 0 since we have now unmapped it.
            map.unmap(&mut offset_table, map.start_addr, map.end_addr)
                .expect("vm_clear: unexpected error while unmapping");

            cursor.remove_current();
        }
    }

    fn munmap(&mut self, address: VirtAddr, size: usize) -> bool {
        let start = address.align_up(Size4KiB::SIZE);
        let end = (address + size).align_up(Size4KiB::SIZE);

        let mut cursor = self.mappings.cursor_front_mut();
        let mut success = false;

        let mut address_space = AddressSpace::this();
        let mut offset_table = address_space.offset_page_table();

        log::debug!("unmapping {:?}..{:?}", start, end);

        while let Some(map) = cursor.current() {
            if map.end_addr <= start {
                cursor.move_next();
            } else {
                match map.unmap(&mut offset_table, start, end) {
                    Ok(result) => match result {
                        UnmapResult::None => return success,
                        UnmapResult::Start => return true,

                        UnmapResult::Full => {
                            success = true;
                            cursor.remove_current();
                        }

                        UnmapResult::Parital(mapping) => {
                            cursor.insert_after(mapping);
                            return true;
                        }

                        UnmapResult::End => {
                            success = true;
                            cursor.move_next();
                        }
                    },

                    Err(_) => return false,
                }
            }
        }

        success
    }

    fn fork_from(&mut self, parent: &Vm) {
        let data = parent.inner.lock();

        // Copy over all of the mappings from the parent into the child.
        self.mappings = data.mappings.clone();
    }
}

pub struct Vm {
    inner: Mutex<VmProtected>,
}

impl Vm {
    /// Creates a new instance of VM.
    pub(super) fn new() -> Self {
        Self {
            inner: Mutex::new(VmProtected::new()),
        }
    }

    pub fn mmap(
        &self,
        address: VirtAddr,
        size: usize,
        protection: MMapProt,
        flags: MMapFlags,
        offset: usize,
        file: Option<DirCacheItem>,
    ) -> Option<VirtAddr> {
        self.inner
            .lock()
            .mmap(address, size, protection, flags, offset, file)
    }

    pub fn munmap(&self, address: VirtAddr, size: usize) -> bool {
        self.inner.lock().munmap(address, size)
    }

    pub(super) fn fork_from(&self, parent: &Vm) {
        self.inner.lock().fork_from(parent)
    }

    /// Mapping the provided `bin` file into the VM.
    pub fn load_bin(&self, bin: DirCacheItem) -> Result<LoadedBinary, ElfParseError> {
        let header = parse_elf_header(bin.clone())?;
        let phdr_iter = ProgramHeaderIter::new(header, bin.clone());

        let load_offset = VirtAddr::new(
            if header.pt2.type_().as_type() == header::Type::SharedObject {
                0x40000000u64
            } else {
                0u64
            },
        );

        let mut entry_point = load_offset + header.pt2.entry_point();

        log::debug!("entry point: {:#x}", entry_point);
        log::debug!("entry point type: {:?}", header.pt2.type_().as_type());

        let mut base_addr = VirtAddr::zero();

        for header in phdr_iter {
            let header_type = header
                .get_type()
                .expect("Failed to get program header type");

            let header_flags = header.flags();

            if header_type == xmas_elf::program::Type::Load {
                let virtual_start = VirtAddr::new(header.virtual_addr()).align_down(Size4KiB::SIZE)
                    + load_offset.as_u64();

                if base_addr == VirtAddr::zero() {
                    base_addr = virtual_start;
                }

                let virtual_end = VirtAddr::new(header.virtual_addr() + header.mem_size())
                    .align_up(Size4KiB::SIZE)
                    + load_offset.as_u64();

                let virtual_fend = VirtAddr::new(header.virtual_addr() + header.file_size())
                    + load_offset.as_u64();

                let data_size = virtual_fend - virtual_start;
                let file_offset = align_down(header.offset(), Size4KiB::SIZE);
                let size = (virtual_end - virtual_start) as usize;

                let mut prot = MMapProt::empty();

                if header_flags.is_read() {
                    prot.insert(MMapProt::PROT_READ);
                }

                prot.insert(MMapProt::PROT_WRITE);

                if header_flags.is_execute() {
                    prot.insert(MMapProt::PROT_EXEC);
                }

                /*
                 * The last non-bss frame of the segment consists partly of data and partly of bss
                 * memory, which must be zeroed. Unfortunately, the file representation might have
                 * reused the part of the frame that should be zeroed to store the next segment. This
                 * means that we can't simply overwrite that part with zeroes, as we might overwrite
                 * other data this way.
                 *
                 * Example:
                 *
                 *   XXXXXXXXXXXXXXX000000YYYYYYY000ZZZZZZZZZZZ     virtual memory (XYZ are data)
                 *   |·············|     /·····/   /·········/
                 *   |·············| ___/·····/   /·········/
                 *   |·············|/·····/‾‾‾   /·········/
                 *   |·············||·····|/·̅·̅·̅·̅·̅·····/‾‾‾‾
                 *   XXXXXXXXXXXXXXXYYYYYYYZZZZZZZZZZZ              file memory (zeros are not saved)
                 *   '       '       '       '        '
                 *   The areas filled with dots (`·`) indicate a mapping between virtual and file
                 *   memory. We see that the data regions `X`, `Y`, `Z` have a valid mapping, while
                 *   the regions that are initialized with 0 have not.
                 *
                 *   The ticks (`'`) below the file memory line indicate the start of a new frame. We
                 *   see that the last frames of the `X` and `Y` regions in the file are followed
                 *   by the bytes of the next region. So we can't zero these parts of the frame
                 *   because they are needed by other memory regions.
                 */
                let address = self
                    .inner
                    .lock()
                    .mmap(
                        virtual_start,
                        size,
                        prot,
                        MMapFlags::MAP_PRIVATE | MMapFlags::MAP_FIXED | MMapFlags::MAP_ANONYOMUS,
                        0,
                        None,
                    )
                    .expect("load_bin: failed to memory map ELF header");

                let buffer = unsafe {
                    core::slice::from_raw_parts_mut::<u8>(address.as_mut_ptr(), data_size as usize)
                };

                bin.inode()
                    .read_at(file_offset as usize, buffer)
                    .expect("load_bin: failed to read at offset");

                if !header.flags().is_write() {
                    // TODO: Update the protection flags to remove the writable flag.
                }
            } else if header_type == xmas_elf::program::Type::Tls {
            } else if header_type == xmas_elf::program::Type::Interp {
                let ld = fs::lookup_path(fs::Path::new("/usr/lib/ld.so")).unwrap();

                let res = self.load_bin(ld)?;
                entry_point = res.entry_point;
            }
        }

        Ok(LoadedBinary {
            header,
            entry_point,

            base_addr,
        })
    }

    /// Clears and unmaps all of the mappings in the VM.
    pub(super) fn clear(&self) {
        self.inner.lock().clear()
    }

    /// This function is responsible for handling page faults occured in
    /// user mode. It determines the address, the reason of the page fault
    /// and then passes it off to one of the appropriate page fault handlers.
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
                    mmap.protection,
                    mmap.flags,
                    file.offset,
                    file.size,
                );
            } else {
                log::debug!(
                    "{:?}..{:?} => {:?}, {:?}",
                    mmap.start_addr,
                    mmap.end_addr,
                    mmap.protection,
                    mmap.flags,
                );
            }
        }
    }
}

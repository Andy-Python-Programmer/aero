// Copyright (C) 2021-2024 The Aero Project Developers.
//
// This file is part of The Aero Project.
//
// Aero is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Aero is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Aero. If not, see <https://www.gnu.org/licenses/>.

use core::fmt::Write;
use core::ops::Range;

use aero_syscall::{MMapFlags, MMapProt, OpenFlags};

use alloc::boxed::Box;
use alloc::collections::linked_list::CursorMut;
use alloc::collections::LinkedList;

use alloc::sync::Arc;
use hashbrown::HashMap;
use xmas_elf::header::*;
use xmas_elf::program::*;
use xmas_elf::*;

use crate::arch::task::userland_last_address;
use crate::fs::block::PageCacheItem;
use crate::fs::cache::{DirCacheImpl, DirCacheItem};
use crate::fs::file_table::FileHandle;
use crate::fs::inode::MMapPage;
use crate::fs::{FileSystemError, Path};
use crate::mem::paging::*;
use crate::mem::AddressSpace;
use crate::{fs, mem};

use crate::syscall::ExecArgs;
use crate::utils::sync::BMutex;

bitflags::bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq)]
    pub struct VmFlag: u8 {
        // currently active flags
        const READ      = MMapProt::PROT_READ.bits() as _;
        const WRITE     = MMapProt::PROT_WRITE.bits() as _;
        const EXEC      = MMapProt::PROT_EXEC.bits() as _;

        // limits
        const MAY_READ  = 1 << 3;
        const MAY_WRITE = 1 << 4;
        const MAY_EXEC  = 1 << 5;

        const SHARED    = 1 << 6;
    }
}

const VM_PROT_MASK: VmFlag =
    VmFlag::from_bits_retain(VmFlag::READ.bits() | VmFlag::WRITE.bits() | VmFlag::EXEC.bits());

impl From<MMapProt> for VmFlag {
    #[inline]
    fn from(value: MMapProt) -> Self {
        Self::from_bits_retain(value.bits().try_into().unwrap())
    }
}

impl From<VmFlag> for PageTableFlags {
    fn from(flags: VmFlag) -> Self {
        let mut value = PageTableFlags::empty();

        if !flags.contains(VmFlag::EXEC) {
            value.insert(PageTableFlags::NO_EXECUTE);
        }

        if flags.contains(VmFlag::WRITE) {
            value.insert(PageTableFlags::WRITABLE);
        }

        value
    }
}

const ELF_HEADER_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];

const ELF_PT1_SIZE: usize = core::mem::size_of::<HeaderPt1>();
const ELF_PT2_64_SIZE: usize = core::mem::size_of::<HeaderPt2_<P64>>();

#[derive(Debug)]
pub enum ElfLoadError {
    /// Unexpected file system error occurred on an IO operation on the file.
    IOError(FileSystemError),
    /// The PT1 header has an invalid magic number.
    InvalidMagic,
    /// The ELF header contains an invalid class.
    InvalidClass,
    /// The provided program header index is invalid.
    InvalidProgramHeaderIndex,
    /// Unexpected file system error occurred when memory mapping an
    /// ELF segment.
    MemoryMapError,
}

fn parse_elf_header<'header>(file: &DirCacheItem) -> Result<Header<'header>, ElfLoadError> {
    // 1. Read the ELF PT1 header:
    let pt1_hdr_slice = Box::leak(mem::alloc_boxed_buffer::<u8>(ELF_PT1_SIZE));

    file.inode()
        .read_at(OpenFlags::empty(), 0, pt1_hdr_slice)
        .map_err(ElfLoadError::IOError)?;

    let pt1_header: &'header _ = unsafe { &*pt1_hdr_slice.as_ptr().cast::<HeaderPt1>() };

    // 2. Ensure that the header has the correct magic number:
    if pt1_header.magic != ELF_HEADER_MAGIC {
        return Err(ElfLoadError::InvalidMagic);
    }

    let pt2_header = match pt1_header.class() {
        // 3. Read the 64-bit PT2 header:
        Class::SixtyFour => {
            let pt2_hdr_slice = Box::leak(mem::alloc_boxed_buffer::<u8>(ELF_PT2_64_SIZE));

            file.inode()
                .read_at(OpenFlags::empty(), ELF_PT1_SIZE, pt2_hdr_slice)
                .map_err(ElfLoadError::IOError)?;

            let pt2_header_ptr = pt2_hdr_slice.as_ptr();
            let pt2_header: &'header _ = unsafe { &*pt2_header_ptr.cast::<HeaderPt2_<P64>>() };

            Ok(HeaderPt2::Header64(pt2_header))
        }

        // 3. Read the 32-bit PT2 header:
        Class::ThirtyTwo => {
            unimplemented!("parse_elf_header: 32-bit executables are not implemented")
        }

        // SAFETY: ensure the PT1 header has a valid class.
        Class::None | Class::Other(_) => Err(ElfLoadError::InvalidClass),
    }?;

    Ok(Header {
        pt1: pt1_header,
        pt2: pt2_header,
    })
}

fn parse_program_header<'pheader>(
    file: &DirCacheItem,
    header: Header<'pheader>,
    index: u16,
) -> Result<ProgramHeader<'pheader>, ElfLoadError> {
    let pt2 = &header.pt2;

    // SAFETY: ensure that the provided program header index is valid.
    if !(index < pt2.ph_count() && pt2.ph_offset() > 0 && pt2.ph_entry_size() > 0) {
        return Err(ElfLoadError::InvalidProgramHeaderIndex);
    }

    // 1. Calculate the start offset and size of the program header:
    let start = pt2.ph_offset() as usize + index as usize * pt2.ph_entry_size() as usize;
    let size = pt2.ph_entry_size() as usize;

    // 2. Read the 64-bit program header:
    let phdr_buffer = Box::leak(mem::alloc_boxed_buffer::<u8>(size));

    file.inode()
        .read_at(OpenFlags::empty(), start, phdr_buffer)
        .map_err(ElfLoadError::IOError)?;

    let phdr_ptr = phdr_buffer.as_ptr();

    match header.pt1.class() {
        // 3. Cast and return the 64-bit program header:
        Class::SixtyFour => {
            let phdr: &'pheader _ = unsafe { &*phdr_ptr.cast::<ProgramHeader64>() };
            Ok(ProgramHeader::Ph64(phdr))
        }

        // 3. Cast and return the 32-bit program header:
        Class::ThirtyTwo => {
            let phdr: &'pheader _ = unsafe { &*phdr_ptr.cast::<ProgramHeader32>() };
            Ok(ProgramHeader::Ph32(phdr))
        }

        // SAFETY: ensure the PT1 header has a valid class.
        Class::None | Class::Other(_) => Err(ElfLoadError::InvalidClass),
    }
}

struct Shebang {
    interpreter: DirCacheItem,
    argument: String,
}

impl Shebang {
    fn new<P: AsRef<Path>>(path: P, argument: String) -> Result<Self, ElfLoadError> {
        let interpreter = fs::lookup_path(path.as_ref()).map_err(ElfLoadError::IOError)?;

        Ok(Self {
            interpreter,
            argument,
        })
    }
}

/// Returns [`true`] if the provided executable (`bin`) contains a shebang
/// at the start.
fn contains_shebang(bin: &DirCacheItem) -> Result<bool, ElfLoadError> {
    let shebang = &mut [0u8; 2];

    bin.inode()
        .read_at(OpenFlags::empty(), 0, shebang)
        .map_err(ElfLoadError::IOError)?;

    Ok(shebang[0] == b'#' && shebang[1] == b'!')
}

fn parse_shebang(bin: &DirCacheItem) -> Result<Option<Shebang>, ElfLoadError> {
    if !contains_shebang(bin)? {
        return Ok(None);
    }

    // Syntax: #![whitespace]interpreter_path [single-argument][new-line]
    //
    // NOTE: We set the position to `2` since we skip the `#!` prefix.
    let mut idx = 2;

    let read_at_index = |idx: usize| -> Result<char, ElfLoadError> {
        let c = &mut [0u8; 1];

        bin.inode()
            .read_at(OpenFlags::empty(), idx, c)
            .map_err(ElfLoadError::IOError)?;

        Ok(c[0] as char)
    };

    // 1. check for the optional whitespace (ignore it):
    if read_at_index(idx)? == ' ' {
        idx += 1;
    }

    // we build the string with `16` capicity to avoid reallocations.
    let mut path = String::with_capacity(16);
    let mut arg = String::with_capacity(16);

    // 2. parse the interpreter path:
    loop {
        let char = read_at_index(idx)?;

        if char == ' ' {
            idx += 1;
            break;
        } else if char == '\n' {
            // there is no argument, early return:
            return Ok(Some(Shebang::new(path, arg)?));
        }

        idx += 1;
        path.write_char(char)
            .expect("parse_shebang: internal error");
    }

    // 3. parse the argument:
    loop {
        let char = read_at_index(idx)?;
        idx += 1;

        if char == '\n' || char == ' ' {
            return Ok(Some(Shebang::new(path, arg)?));
        }

        arg.write_char(char).expect("parse_shebang: internal error");
    }
}

pub struct Elf<'this> {
    pub header: Header<'this>,
    pub file: DirCacheItem,
}

impl<'this> Elf<'this> {
    fn new(file: DirCacheItem) -> Result<Self, ElfLoadError> {
        let header = parse_elf_header(&file)?;
        Ok(Self { header, file })
    }

    fn program_iter(&self) -> ProgramHeaderIter<'this> {
        ProgramHeaderIter::new(self.header, self.file.clone())
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
        let result = parse_program_header(&self.file, self.header, self.next_index as u16).ok();

        // Increment the next index.
        self.next_index += 1;
        result
    }
}

enum UnmapResult {
    None,
    Partial(Mapping),
    Full,
    Start,
    End,
}

pub struct LoadedBinary<'header> {
    pub elf: Elf<'header>,

    pub entry_point: VirtAddr,
    pub base_addr: VirtAddr,

    pub argv: Option<ExecArgs>,
    pub envv: Option<ExecArgs>,
}

#[derive(Clone)]
pub struct MMapFile {
    offset: usize,
    file: DirCacheItem,
    size: usize,
    mappings: HashMap<VirtAddr, PageCacheItem>,
}

impl MMapFile {
    #[inline]
    fn new(file: DirCacheItem, offset: usize, size: usize) -> Self {
        Self {
            offset,
            file,
            size,
            mappings: HashMap::new(),
        }
    }
}

#[derive(Clone)]
pub struct Mapping {
    flags: VmFlag,

    pub start_addr: VirtAddr,
    pub end_addr: VirtAddr,

    pub file: Option<MMapFile>,
    refresh_flags: bool,
}

impl Mapping {
    pub fn set_protection(&mut self, protection: MMapProt) -> aero_syscall::Result<()> {
        if (protection.contains(MMapProt::PROT_READ) && !self.flags.contains(VmFlag::MAY_READ))
            || (protection.contains(MMapProt::PROT_WRITE)
                && !self.flags.contains(VmFlag::MAY_WRITE))
            || (protection.contains(MMapProt::PROT_EXEC) && !self.flags.contains(VmFlag::MAY_EXEC))
        {
            return Err(aero_syscall::SyscallError::EACCES);
        }

        self.flags = (self.flags & !VM_PROT_MASK) | protection.into();
        Ok(())
    }

    #[inline]
    pub fn protection(&self) -> VmFlag {
        self.flags & VM_PROT_MASK
    }

    /// Handler routine for private anonymous pages. Since its an anonymous page is not
    /// backed by a file, we have to alloctate a frame and map it at the faulted address.
    fn handle_pf_private_anon(
        &mut self,
        offset_table: &mut OffsetPageTable,
        reason: PageFaultErrorCode,
        address: VirtAddr,
    ) -> bool {
        let addr_aligned = address.align_down(Size4KiB::SIZE);

        if !reason.contains(PageFaultErrorCode::PROTECTION_VIOLATION) {
            let frame: PhysFrame =
                PhysFrame::containing_address(pmm_alloc(BuddyOrdering::Size4KiB));

            unsafe {
                offset_table.map_to(
                    Page::containing_address(addr_aligned),
                    frame,
                    // NOTE: We dont need to remove the writeable flag from this mapping, since
                    // the writeable flag will be removed from the parent and child on fork so,
                    // the mapping gets copied on write.
                    PageTableFlags::USER_ACCESSIBLE | PageTableFlags::PRESENT | self.flags.into(),
                )
            }
            .expect("Failed to identity map userspace private mapping")
            .flush();

            true
        } else if reason.contains(PageFaultErrorCode::CAUSED_BY_WRITE) {
            self.handle_cow(offset_table, addr_aligned, false)
        } else {
            if !self.refresh_flags {
                return false;
            }

            unsafe {
                // The page is present but most likely the flags need to be updated after
                // mprotect(2).
                let page: Page<Size4KiB> = Page::containing_address(address);
                offset_table
                    .update_flags(
                        page,
                        PageTableFlags::USER_ACCESSIBLE
                            | PageTableFlags::PRESENT
                            | self.flags.into(),
                    )
                    .unwrap()
                    .flush();
            }

            self.refresh_flags = false;
            true
        }
    }

    /// Handler routine for pages backed by a file. This function will allocate a frame and
    /// read a page-sized amount from the disk into the allocated frame. Then it maps
    /// the allocated frame at the faulted address.
    fn handle_pf_file(
        &mut self,
        offset_table: &mut OffsetPageTable,
        reason: PageFaultErrorCode,
        addr: VirtAddr,
    ) -> bool {
        if let Some(file) = self.file.as_ref() {
            let offset = align_down(
                (addr - self.start_addr) + file.offset as u64,
                Size4KiB::SIZE,
            );

            let addr = addr.align_down(Size4KiB::SIZE);
            let size = Size4KiB::SIZE.min(file.size as u64 - (addr - self.start_addr));

            return if self.flags.contains(VmFlag::SHARED) {
                self.handle_pf_shared_file(offset_table, reason, addr, offset as _, size as _)
            } else {
                self.handle_pf_private_file(offset_table, reason, addr, offset as _, size as _)
            };
        }

        false
    }

    fn handle_pf_private_file(
        &mut self,
        offset_table: &mut OffsetPageTable,
        reason: PageFaultErrorCode,

        addr: VirtAddr,
        offset: usize,
        size: usize,
    ) -> bool {
        let mmap_file = self.file.as_mut().unwrap();
        let page_cache = if let MMapPage::PageCache(page_cache) =
            mmap_file.file.inode().mmap_v2(offset).unwrap()
        {
            page_cache
        } else {
            todo!()
        };

        if !reason.contains(PageFaultErrorCode::PROTECTION_VIOLATION)
            && !reason.contains(PageFaultErrorCode::CAUSED_BY_WRITE)
        {
            let frame = if size == Size4KiB::SIZE as usize {
                page_cache.page()
            } else {
                // The end needs to be zeroed out so we cannot directly map the cached page.
                let page: Page = Page::containing_address(page_cache.data_addr().as_hhdm_virt());

                let new_frame: PhysFrame = PhysFrame::containing_address(
                    FRAME_ALLOCATOR
                        .alloc_zeroed(Size4KiB::SIZE as usize)
                        .unwrap(),
                );

                let new_slice = new_frame.as_slice_mut::<u8>();
                new_slice[..size].copy_from_slice(unsafe {
                    core::slice::from_raw_parts(page.start_address().as_ptr::<u8>(), size)
                });

                new_frame
            };

            unsafe {
                offset_table.map_to(
                    Page::containing_address(addr),
                    frame,
                    PageTableFlags::PRESENT
                        | PageTableFlags::USER_ACCESSIBLE
                        | (self.flags & !VmFlag::WRITE).into(),
                )
            }
            .expect("failed to map allocated frame for private file read")
            .flush();

            true
        } else if !reason.contains(PageFaultErrorCode::PROTECTION_VIOLATION)
            && reason.contains(PageFaultErrorCode::CAUSED_BY_WRITE)
        {
            // We are writing to private file mapping so copy the content of the page.
            let frame = mmap_file
                .file
                .inode()
                .mmap(offset, size, MMapFlags::empty())
                .expect("handle_pf_file: file does not support mmap");

            unsafe {
                offset_table.map_to(
                    Page::containing_address(addr),
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE | self.flags.into(),
                )
            }
            .expect("failed to map allocated frame for private file read")
            .flush();

            assert!(mmap_file.mappings.get(&addr).is_none());
            true
        } else if reason.contains(PageFaultErrorCode::CAUSED_BY_WRITE)
            && reason.contains(PageFaultErrorCode::PROTECTION_VIOLATION)
        {
            if self.handle_cow(offset_table, addr, true) {
                self.file.as_mut().unwrap().mappings.remove(&addr);
                return true;
            }

            false
        } else {
            log::error!("    - present page read failed");
            false
        }
    }

    fn handle_pf_shared_file(
        &mut self,
        offset_table: &mut OffsetPageTable,
        reason: PageFaultErrorCode,

        addr: VirtAddr,
        offset: usize,
        _size: usize,
    ) -> bool {
        let mmap_file = self.file.as_mut().unwrap();
        let mmap_page = mmap_file.file.inode().mmap_v2(offset).unwrap();

        if reason.contains(PageFaultErrorCode::PROTECTION_VIOLATION) {
            if reason.contains(PageFaultErrorCode::CAUSED_BY_WRITE) {
                return false;
            }
        } else if let MMapPage::PageCache(page_cache) = &mmap_page {
            mmap_file.mappings.insert(addr, page_cache.clone());
        }

        match mmap_page {
            MMapPage::PageCache(page_cache) => {
                if reason.contains(PageFaultErrorCode::CAUSED_BY_WRITE) {
                    unsafe {
                        offset_table.map_to(
                            Page::containing_address(addr),
                            page_cache.page(),
                            PageTableFlags::PRESENT
                                | PageTableFlags::USER_ACCESSIBLE
                                | self.flags.into(),
                        )
                    }
                    .unwrap()
                    .flush();

                    page_cache.mark_dirty();
                } else {
                    unsafe {
                        offset_table.map_to(
                            Page::containing_address(addr),
                            page_cache.page(),
                            PageTableFlags::PRESENT
                                | PageTableFlags::USER_ACCESSIBLE
                                | (self.flags & !VmFlag::WRITE).into(),
                        )
                    }
                    .unwrap()
                    .flush();
                }
            }

            MMapPage::Direct(frame) => {
                unsafe {
                    offset_table.map_to(
                        Page::containing_address(addr),
                        frame,
                        PageTableFlags::PRESENT
                            | PageTableFlags::USER_ACCESSIBLE
                            | self.flags.into(),
                    )
                }
                .unwrap()
                .flush();
            }
        }

        true
    }

    /// Copies the contents of the `page` page to a newly allocated frame and maps it to
    /// the `page` page with the provided `protection` protection flags.
    fn map_copied(
        offset_table: &mut OffsetPageTable,
        page: Page<Size4KiB>,
        flags: VmFlag,
    ) -> Result<(), MapToError<Size4KiB>> {
        // Allocate a new frame to hold the contents.
        let new_frame: PhysFrame<Size4KiB> = FRAME_ALLOCATOR
            .allocate_frame()
            .expect("map_copied: failed to allocate frame");

        let old_slice = unsafe {
            let ptr = page.start_address().as_ptr::<u8>();
            core::slice::from_raw_parts(ptr, Size4KiB::SIZE as _)
        };

        let new_slice = new_frame.as_slice_mut::<u8>();

        // Copy the contents from the old frame to the newly allocated frame.
        new_slice.copy_from_slice(old_slice);

        // Re-map the page to the newly allocated frame and with the provided
        // protection flags.
        offset_table.unmap(page).unwrap().1.ignore();

        // NOTE: We operate on an active page table, so we flush the changes.
        unsafe {
            offset_table
                .map_to(
                    page,
                    new_frame,
                    PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE | flags.into(),
                )?
                .flush();
        }

        Ok(())
    }

    /// Handler routine for a COW (Copy-On-Write) pages. A COW page is shared between multiple
    /// processes until a write occurs after which a private copy is made for the writing
    /// process. A COW page is recognised because the VMA for the region is marked writable even
    /// though the individual page table entry is not.
    ///
    /// ## Panics
    /// * The provided `address` is not aligned to a page boundary.
    fn handle_cow(
        &mut self,
        offset_table: &mut OffsetPageTable,
        address: VirtAddr,
        copy: bool,
    ) -> bool {
        debug_assert!(address.is_aligned(Size4KiB::SIZE));

        let page: Page<Size4KiB> = Page::containing_address(address);

        if let TranslateResult::Mapped { frame, .. } = offset_table.translate(address) {
            let phys_addr = frame.start_address();

            if let Some(vm_frame) = phys_addr.as_vm_frame() {
                if vm_frame.ref_count() > 1 || copy {
                    // This page is used by more then one process, so make it a private copy.
                    Self::map_copied(offset_table, page, self.flags).unwrap();
                } else {
                    // This page is used by only one process, so make it writable.
                    unsafe {
                        offset_table.update_flags(
                            page,
                            PageTableFlags::PRESENT
                                | PageTableFlags::USER_ACCESSIBLE
                                | self.flags.into(),
                        )
                    }
                    .unwrap()
                    .flush();
                }

                return true;
            }
        }

        false
    }

    fn unmap(
        &mut self,
        offset_table: &mut OffsetPageTable,
        start: VirtAddr,
        end: VirtAddr,
    ) -> Result<UnmapResult, UnmapError> {
        let mut unmap_range_inner = |range: Range<VirtAddr>| -> Result<(), UnmapError> {
            for addr in range.step_by(Size4KiB::SIZE as usize) {
                let page: Page = Page::containing_address(addr);
                match offset_table.unmap(page) {
                    Ok((_, flusher)) => flusher.flush(),
                    Err(UnmapError::PageNotMapped) => {}
                    Err(e) => return Err(e),
                }
            }

            Ok(())
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
                start_addr: end,
                end_addr: end + (self.end_addr - end),
                file: new_file,
                refresh_flags: true,
                flags: self.flags,
            };

            self.end_addr = start;
            Ok(UnmapResult::Partial(new_mapping))
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
            self.end_addr = start;
            Ok(UnmapResult::End)
        }
    }

    fn size(&self) -> usize {
        (self.end_addr - self.start_addr) as usize
    }

    fn split(&self, start: VirtAddr, end: VirtAddr) -> (Mapping, Mapping, Mapping) {
        assert!(start > self.start_addr && end < self.end_addr);

        let mut left = self.clone();
        left.end_addr = start;

        let mut mid = self.clone();
        mid.start_addr = start;
        mid.end_addr = end;

        let mut right = self.clone();
        right.start_addr = end;

        if self.file.is_some() {
            left.file.as_mut().unwrap().size = left.size();
            mid.file.as_mut().unwrap().offset += left.size();
            mid.file.as_mut().unwrap().size = mid.size();
            right.file.as_mut().unwrap().offset += mid.size();
            right.file.as_mut().unwrap().size = right.size();
        }

        (left, mid, right)
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
            if map.protection().is_empty() {
                return false;
            }

            if reason.contains(PageFaultErrorCode::CAUSED_BY_WRITE)
                && !map.flags.contains(VmFlag::WRITE)
            {
                return false;
            }

            if reason.contains(PageFaultErrorCode::INSTRUCTION_FETCH)
                && !map.flags.contains(VmFlag::EXEC)
            {
                return false;
            }

            let mut address_space = AddressSpace::this();
            let mut offset_table = address_space.offset_page_table();

            match (!map.flags.contains(VmFlag::SHARED), map.file.is_none()) {
                (true, true) => {
                    map.handle_pf_private_anon(&mut offset_table, reason, accessed_address)
                }

                (true | false, false) => {
                    map.handle_pf_file(&mut offset_table, reason, accessed_address)
                }

                (false, true) => unreachable!("shared and anonymous mapping"),
            }
        } else {
            log::trace!("mapping not found for address: {:#x}", accessed_address);
            self.log();

            // else the mapping does not exist, so return false.
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
            } else if address + size > map.start_addr {
                return None;
            } else {
                break;
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

        let mut cursor = self.mappings.cursor_back_mut();

        // Search the mappings starting at the current cursor position for a big
        // enough hole for where the address is above the provided `address`. A hole is
        // big enough if it can hold the requested `size`. We use the first fit strategy,
        // so it breaks as soon as a big enough hole is found.
        while let Some(map) = cursor.current() {
            let map_start = map.start_addr;

            if map.start_addr < address {
                cursor.move_next();
            } else if let Some(pmap) = cursor.peek_prev() {
                let start = core::cmp::max(address, pmap.end_addr);
                let hole = map_start.as_u64() - start.as_u64();

                if hole as usize >= size {
                    return Some((start, cursor));
                }

                // The hole is too small
                cursor.move_next();
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

        None
    }

    fn mmap(
        &mut self,
        address: VirtAddr,
        size: usize,
        flags: MMapFlags,
        offset: usize,
        file: Option<DirCacheItem>,
        vm_flags: VmFlag,
    ) -> Option<VirtAddr> {
        let z = file.clone();

        // Offset is required to be a multiple of page size.
        if (offset as u64 & (Size4KiB::SIZE - 1)) != 0 {
            log::warn!("mmap: offset is not a multiple of page size");
            return None;
        }

        // The provided size should be non-zero.
        if size == 0 {
            log::warn!("mmap: size is zero");
            return None;
        }

        if file.is_some() {
            if flags.contains(MMapFlags::MAP_ANONYOMUS) {
                log::warn!("mmap: cannot map a file with the anonymous flag");
                return None;
            }
        } else {
            if !flags.contains(MMapFlags::MAP_ANONYOMUS) {
                log::warn!("mmap: mappings not backed by a file cannot be anonymous");
                return None;
            }

            if flags.contains(MMapFlags::MAP_SHARED) {
                log::warn!("mmap: anonymous mappings cannot be shared");
                return None;
            }
        }

        // TODO: align_up may overflow. return if size_aligned == 0
        let size_aligned = align_up(size as _, Size4KiB::SIZE);

        let x = if address == VirtAddr::zero() {
            // We need to find a free mapping above 0x7000_0000_0000.
            self.find_any_above(VirtAddr::new(0x7000_0000_0000), size_aligned as _)
        } else if flags.contains(MMapFlags::MAP_FIXED) {
            if !address.is_aligned(Size4KiB::SIZE) {
                log::warn!("mmap: fixed mapping address is not page aligned");
                return None;
            }

            if (address + size_aligned) > userland_last_address() {
                log::warn!("mmap: fixed mapping address is out of range");
                return None;
            }

            self.munmap(address, size_aligned as usize); // Unmap any existing mappings.
            self.find_fixed_mapping(address, size_aligned as _)
        } else {
            self.find_any_above(address, size)
        }
        .map(|(addr, mut cursor)| {
            // Merge same mappings instead of creating a new one.
            if let Some(prev) = cursor.peek_prev() {
                if prev.end_addr == addr
                    && prev.flags == vm_flags
                    && prev.file.is_none()
                    && file.is_none()
                {
                    prev.end_addr = addr + size_aligned;
                    return addr;
                }
            }

            cursor.insert_before(Mapping {
                start_addr: addr,
                end_addr: addr + size_aligned,

                file: file.map(|f| MMapFile::new(f, offset, size)),
                refresh_flags: true,
                flags: vm_flags,
            });

            addr
        });

        if x.is_none() {
            log::warn!("mmap failed");
            self.log();

            dbg!(
                address,
                size,
                vm_flags,
                flags,
                offset,
                z.map(|f| f.absolute_path())
            );

            crate::unwind::unwind_stack_trace();
        }

        x
    }

    fn log(&self) {
        for mmap in &self.mappings {
            if let Some(file) = mmap.file.as_ref() {
                log::debug!(
                    "{:?}..{:?} => {:?} (offset={:#x}, size={:#x})",
                    mmap.start_addr,
                    mmap.end_addr,
                    mmap.flags,
                    file.offset,
                    file.size,
                );
            } else {
                log::debug!(
                    "{:?}..{:?} => {:?}",
                    mmap.start_addr,
                    mmap.end_addr,
                    mmap.flags,
                );
            }
        }
    }

    fn load_bin<'header>(
        &mut self,
        bin: &DirCacheItem,
        argv: Option<ExecArgs>,
        envv: Option<ExecArgs>,
    ) -> Result<LoadedBinary<'header>, ElfLoadError> {
        // check for a shebang before proceeding.
        if let Some(shebang) = parse_shebang(bin)? {
            log::debug!(
                "shebang: (interpreter={}, argument={})",
                shebang.interpreter.absolute_path(),
                shebang.argument
            );

            let mut largv = ExecArgs::default();

            largv.push(shebang.interpreter.absolute_path().as_bytes());

            if !shebang.argument.is_empty() {
                largv.push(shebang.argument.as_bytes());
            }

            largv.push(bin.absolute_path().as_bytes());

            if let Some(argv) = argv {
                largv.extend(&argv.inner[1..])
            }

            return self.load_bin(&shebang.interpreter, Some(largv), envv);
        }

        let elf = Elf::new(bin.clone())?;
        let header = &elf.header;

        let load_offset = VirtAddr::new(
            if header.pt2.type_().as_type() == header::Type::SharedObject {
                0x4000_0000_u64
            } else {
                0u64
            },
        );

        let mut entry_point = load_offset + header.pt2.entry_point();

        log::debug!("entry point: {:#x}", entry_point);
        log::debug!("entry point type: {:?}", header.pt2.type_().as_type());

        let mut base_addr = VirtAddr::zero();

        for header in elf.program_iter() {
            let header_type = header
                .get_type()
                .expect("load_bin: failed to get program header type");

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
                let aligned_data_size = align_up(data_size, Size4KiB::SIZE);

                let file_offset = align_down(header.offset(), Size4KiB::SIZE);

                let mut flags = VmFlag::MAY_READ | VmFlag::MAY_WRITE | VmFlag::MAY_EXEC;

                if header_flags.is_read() {
                    flags.insert(VmFlag::READ);
                }

                if header_flags.is_write() {
                    flags.insert(VmFlag::WRITE);
                }

                if header_flags.is_execute() {
                    flags.insert(VmFlag::EXEC);
                }

                #[rustfmt::skip]
                // The last non-bss frame of the segment consists partly of data and partly of bss
                // memory, which must be zeroed. Unfortunately, the file representation might have
                // reused the part of the frame that should be zeroed to store the next segment. This
                // means that we can't simply overwrite that part with zeroes, as we might overwrite
                // other data this way.
                //
                // Example:
                //
                //  XXXXXXXXXXXXXXX000000YYYYYYY000ZZZZZZZZZZZ     virtual memory (XYZ are data)
                //  |·············|     /·····/   /·········/
                //  |·············| ___/·····/   /·········/
                //  |·············|/·····/‾‾‾   /·········/
                //  |·············||·····|/·̅·̅·̅·̅·̅·····/‾‾‾‾
                //  XXXXXXXXXXXXXXXYYYYYYYZZZZZZZZZZZ              file memory (zeros are not saved)
                //  '       '       '       '        '
                //  The areas filled with dots (`·`) indicate a mapping between virtual and file
                //  memory. We see that the data regions `X`, `Y`, `Z` have a valid mapping, while
                //  the regions that are initialized with 0 have not.
                //
                //  The ticks (`'`) below the file memory line indicate the start of a new frame. We
                //  see that the last frames of the `X` and `Y` regions in the file are followed
                //  by the bytes of the next region. So we can't zero these parts of the frame
                //  because they are needed by other memory regions.
                let address = self
                    .mmap(
                        virtual_start,
                        data_size as usize,
                        MMapFlags::MAP_PRIVATE | MMapFlags::MAP_FIXED,
                        file_offset as usize,
                        Some(bin.clone()),
                        flags
                    )
                    .ok_or(ElfLoadError::MemoryMapError)?;

                let virtual_fend = address + aligned_data_size;

                if virtual_fend < virtual_end {
                    let bss_size = virtual_end - virtual_fend;

                    self.mmap(
                        virtual_fend,
                        bss_size as usize,
                        MMapFlags::MAP_PRIVATE | MMapFlags::MAP_ANONYOMUS | MMapFlags::MAP_FIXED,
                        0,
                        None,
                        flags,
                    )
                    .ok_or(ElfLoadError::MemoryMapError)?;
                }
            } else if header_type == xmas_elf::program::Type::Tls {
            } else if header_type == xmas_elf::program::Type::Interp {
                let ld = fs::lookup_path(fs::Path::new("/usr/lib/ld.so")).unwrap();

                let res = self.load_bin(&ld, None, None)?;
                entry_point = res.entry_point;
            }
        }

        Ok(LoadedBinary {
            elf,
            entry_point,

            base_addr,

            argv,
            envv,
        })
    }

    /// Clears all of the mappings without unmapping them. The caller is responsible
    /// for going through the page table and unmapping all of the pages.
    fn clear(&mut self) {
        self.mappings.clear()
    }

    fn munmap(&mut self, address: VirtAddr, size: usize) -> bool {
        let start = address.align_up(Size4KiB::SIZE);
        let end = (address + size).align_up(Size4KiB::SIZE);

        let mut cursor = self.mappings.cursor_front_mut();
        let mut success = false;

        let mut address_space = AddressSpace::this();
        let mut offset_table = address_space.offset_page_table();

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

                        UnmapResult::Partial(mapping) => {
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

    fn mprotect(
        &mut self,
        addr: VirtAddr,
        size: usize,
        prot: MMapProt,
    ) -> aero_syscall::Result<()> {
        let start = addr.align_up(Size4KiB::SIZE);
        let end = (addr + size).align_up(Size4KiB::SIZE);

        let mut cursor = self.mappings.cursor_front_mut();

        while let Some(map) = cursor.current() {
            if map.end_addr <= start {
                cursor.move_next();
            } else if end <= map.start_addr || start >= map.end_addr {
                break;
            } else if start > map.start_addr && end < map.end_addr {
                // The address we want to unmap is in the middle of the region. So we
                // will need to split the mapping and update the end address accordingly.
                let (left, mut mid, right) = map.split(start, end);
                mid.set_protection(prot)?;

                cursor.insert_after(right);
                cursor.insert_after(mid);
                cursor.insert_after(left);
                cursor.remove_current();
                break;
            } else if start <= map.start_addr && end >= map.end_addr {
                // full
                map.set_protection(prot)?;
                cursor.move_next();
            } else if start <= map.start_addr && end < map.end_addr {
                // start
                let mut mapping = map.clone();
                mapping.end_addr = end;
                mapping.set_protection(prot)?;

                map.start_addr = end;
                cursor.insert_before(mapping);
                break;
            } else {
                // end
                let mut mapping = map.clone();
                mapping.start_addr = start;
                mapping.set_protection(prot)?;

                map.end_addr = start;
                cursor.insert_after(mapping);
                cursor.move_next();
            }
        }

        Ok(())
    }

    #[must_use]
    fn fork_from(&mut self, parent: &Vm) -> AddressSpace {
        {
            let parent = parent.inner.lock();
            self.mappings.clone_from(&parent.mappings);
        }

        let mut address_space = AddressSpace::new().unwrap();
        let mut offset_table = address_space.offset_page_table();

        let mut current = AddressSpace::this();
        let mut current = current.offset_page_table();

        for map in self.mappings.iter().filter(|map| {
            // Do not copy page table entries where a page fault can map them correctly.
            !map.flags.contains(VmFlag::SHARED) && map.flags.contains(VmFlag::MAY_WRITE)
        }) {
            offset_table.copy_page_range(&mut current, map.start_addr..=map.end_addr);
        }

        address_space
    }
}

pub struct Vm {
    inner: BMutex<VmProtected>,
}

impl Vm {
    /// Creates a new instance of VM.
    pub(super) fn new() -> Self {
        Self {
            inner: BMutex::new(VmProtected::new()),
        }
    }

    pub fn mmap(
        &self,
        address: VirtAddr,
        size: usize,
        protection: MMapProt,
        flags: MMapFlags,
        offset: usize,
        file: Option<Arc<FileHandle>>,
    ) -> Option<VirtAddr> {
        let mut vm_flags =
            VmFlag::from(protection) | VmFlag::MAY_READ | VmFlag::MAY_WRITE | VmFlag::MAY_EXEC;

        let map_type = flags & (MMapFlags::MAP_SHARED | MMapFlags::MAP_PRIVATE);

        match (map_type, file.as_ref()) {
            (MMapFlags::MAP_SHARED, Some(file)) => {
                vm_flags.insert(VmFlag::SHARED);

                if !file.is_writable() {
                    if protection.contains(MMapProt::PROT_WRITE) {
                        return None; // EACCES
                    }

                    // The mapping is going to be read-only forever so, it can be converted into a
                    // private mapping.
                    vm_flags.remove(VmFlag::MAY_WRITE | VmFlag::SHARED);
                }

                if !file.is_readable() {
                    return None; // EACCES
                }

                // TODO: * check if the filsystem is noexec mounted and remove the MAY_EXEC flag.
                //       * error out if prot contains PROT_EXEC & filesystem is noexec.
            }

            (MMapFlags::MAP_PRIVATE, Some(file)) => {
                if !file.is_readable() {
                    return None; // EACCES
                }

                // TODO: * check if the filsystem is noexec mounted and remove the MAY_EXEC flag.
                //       * error out if prot contains PROT_EXEC & filesystem is noexec.
            }

            (MMapFlags::MAP_SHARED, None) => vm_flags.insert(VmFlag::SHARED),
            _ => {}
        }

        let file = file.map(|file| file.dirnode());
        self.inner
            .lock()
            .mmap(address, size, flags, offset, file, vm_flags)
    }

    pub fn munmap(&self, address: VirtAddr, size: usize) -> bool {
        self.inner.lock().munmap(address, size)
    }

    pub fn mprotect(&self, ptr: VirtAddr, size: usize, prot: MMapProt) {
        self.inner.lock().mprotect(ptr, size, prot).unwrap()
    }

    pub(super) fn fork_from(&self, parent: &Vm) -> AddressSpace {
        self.inner.lock().fork_from(parent)
    }

    pub fn log(&self) {
        self.inner.lock().log()
    }

    /// Mapping the provided `bin` file into the VM.
    pub fn load_bin(
        &self,
        bin: &DirCacheItem,
        argv: Option<ExecArgs>,
        envv: Option<ExecArgs>,
    ) -> Result<LoadedBinary, ElfLoadError> {
        self.inner.lock().load_bin(bin, argv, envv)
    }

    /// Clears and unmaps all of the mappings in the VM.
    pub(super) fn clear(&self) {
        self.inner.lock().clear()
    }

    /// This function is responsible for handling page faults occurred in
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

    pub fn for_each_mapping<F>(&self, mut f: F)
    where
        F: FnMut(&Mapping),
    {
        for map in &self.inner.lock().mappings {
            f(map);
        }
    }
}

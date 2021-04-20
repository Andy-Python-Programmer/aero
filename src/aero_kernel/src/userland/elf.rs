//! ELF (Executable and Linkable Format) file parsing.

use crate::arch::elf::header;

pub enum ELFError {
    NotEnoughData,
    InvalidMagic,
    InvalidArchitecture,
}

pub struct Elf<'a> {
    pub data: &'a [u8],
    pub header: &'a header::Header,
}

impl<'a> Elf<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, ELFError> {
        if data.len() < header::SIZEOF_EHDR {
            return Err(ELFError::NotEnoughData);
        } else if &data[..header::SELFMAG] != header::ELFMAG {
            return Err(ELFError::InvalidMagic);
        } else if data.get(header::EI_CLASS) != Some(&header::ELFCLASS) {
            return Err(ELFError::InvalidArchitecture);
        }

        let header = unsafe { &*((data.as_ptr() as usize) as *const header::Header) };

        Ok(Self { data, header })
    }
}

use self::{address::VirtualAddress, mapper::Mapper, page::PageTable};

pub mod address;
pub mod frame;
pub mod mapper;
pub mod page;

pub trait PageSize: Copy + Clone + Eq + PartialOrd + Ord {
    /// The page size in bytes.
    const SIZE: u64;
}

pub trait NotGiantPageSize: PageSize {}

macro_rules! impl_size_t {
    ($enum:ident, $size:expr) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        pub enum $enum {}

        impl PageSize for $enum {
            const SIZE: u64 = $size;
        }
    };
}

impl_size_t!(Size4KiB, 4096);
impl_size_t!(Size2MiB, Size4KiB::SIZE * 512);
impl_size_t!(Size1GiB, Size2MiB::SIZE * 512);

impl NotGiantPageSize for Size4KiB {}
impl NotGiantPageSize for Size2MiB {}

#[repr(transparent)]
pub struct ActivePageTable<'mapper>(Mapper<'mapper>);

impl<'mapper> ActivePageTable<'mapper> {
    pub fn new(
        level_4_table: &'mapper mut PageTable,
        physical_memory_offset: VirtualAddress,
    ) -> Self {
        Self(Mapper::new(level_4_table, physical_memory_offset))
    }
}

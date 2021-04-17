use core::fmt;
use core::marker::PhantomData;

use super::{address::PhysicalAddress, PageSize, Size4KiB};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub struct Frame<S: PageSize = Size4KiB> {
    start_address: PhysicalAddress,
    size: PhantomData<S>,
}

impl<S: PageSize> Frame<S> {
    pub fn from_start_address(address: PhysicalAddress) -> Self {
        assert!(address.is_aligned(S::SIZE));

        Self::containing_address(address)
    }

    #[inline]
    pub fn containing_address(address: PhysicalAddress) -> Self {
        Self {
            start_address: address.align_down(S::SIZE),
            size: PhantomData,
        }
    }

    #[inline(always)]
    pub fn start_address(&self) -> PhysicalAddress {
        self.start_address
    }
}

impl<S: PageSize> fmt::Debug for Frame<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "Frame[{}]({:#x})",
            S::SIZE,
            self.start_address().as_u64()
        ))
    }
}

pub trait FrameAllocator {
    fn allocate_frame(&mut self) -> Option<Frame>;
    fn deallocate_frame(&mut self);
}

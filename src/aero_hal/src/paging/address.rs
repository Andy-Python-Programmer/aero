use core::{
    fmt,
    ops::{Add, AddAssign, Sub, SubAssign},
};

use bit_field::BitField;

/// A canonical virtual memory address. The addresses are canonicalized based on
/// the target arch.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VirtualAddress(u64);

impl VirtualAddress {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "x86_64")] {
            /// Create a new canonical virtual memory address.
            pub fn new(address: u64) -> Self {
                match address.get_bits(47..64) {
                    0 | 0x1ffff => Self(address), // The address is canonical.
                    1 => Self(((address << 16) as isize >> 16) as u64), // The address needs to be truncated.
                    _ => panic!("Invalid virtual address"), // Invalid address as 48 to 64 do not have a valid sign extension.
                }
            }
        } else if #[cfg(target_arch = "aarch64")] {
            /// Create a new canonical virtual memory address.
            #[inline(always)]
            pub fn new(address: u64) -> Self {
                match address.get_bits(47..64) {
                    0 | 0xffff  => Self(address), // The address is canonical.
                    _ => panic!("Invalid virtual address") // Invalid address as 48 to 64 do not have a valid sign extension.
                }
            }
        }
    }

    #[inline(always)]
    pub fn as_u64(self) -> u64 {
        self.0
    }

    #[inline(always)]
    pub fn align_down(self, alignment: u64) -> Self {
        Self(align_down(self.0, alignment))
    }

    #[inline(always)]
    pub fn p1_index(&self) -> u16 {
        ((self.0 >> 12) & 0o777) as u16
    }

    #[inline(always)]
    pub fn p2_index(&self) -> u16 {
        ((self.0 >> 12 >> 9) & 0o777) as u16
    }

    #[inline(always)]
    pub fn p3_index(&self) -> u16 {
        ((self.0 >> 12 >> 9 >> 9) & 0o777) as u16
    }

    #[inline(always)]
    pub fn p4_index(&self) -> u16 {
        ((self.0 >> 12 >> 9 >> 9 >> 9) & 0o777) as u16
    }

    #[inline(always)]
    pub fn as_ptr<T>(self) -> *const T {
        self.as_u64() as *const T
    }

    #[inline(always)]
    pub fn as_mut_ptr<T>(self) -> *mut T {
        self.as_ptr::<T>() as *mut T
    }
}

impl fmt::Debug for VirtualAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("VirtualAddress")
            .field(&format_args!("{:#x}", self.0))
            .finish()
    }
}

impl fmt::Binary for VirtualAddress {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Binary::fmt(&self.0, f)
    }
}

impl fmt::LowerHex for VirtualAddress {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

impl fmt::Octal for VirtualAddress {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Octal::fmt(&self.0, f)
    }
}

impl fmt::UpperHex for VirtualAddress {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::UpperHex::fmt(&self.0, f)
    }
}

impl fmt::Pointer for VirtualAddress {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&(self.0 as *const ()), f)
    }
}

impl Add<u64> for VirtualAddress {
    type Output = Self;

    #[inline]
    fn add(self, rhs: u64) -> Self::Output {
        VirtualAddress::new(self.0 + rhs)
    }
}

impl AddAssign<u64> for VirtualAddress {
    #[inline]
    fn add_assign(&mut self, rhs: u64) {
        *self = *self + rhs;
    }
}

#[cfg(target_pointer_width = "64")]
impl Add<usize> for VirtualAddress {
    type Output = Self;

    #[inline]
    fn add(self, rhs: usize) -> Self::Output {
        self + rhs as u64
    }
}

#[cfg(target_pointer_width = "64")]
impl AddAssign<usize> for VirtualAddress {
    #[inline]
    fn add_assign(&mut self, rhs: usize) {
        self.add_assign(rhs as u64)
    }
}

impl Sub<u64> for VirtualAddress {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: u64) -> Self::Output {
        VirtualAddress::new(self.0.checked_sub(rhs).unwrap())
    }
}

impl SubAssign<u64> for VirtualAddress {
    #[inline]
    fn sub_assign(&mut self, rhs: u64) {
        *self = *self - rhs;
    }
}

#[cfg(target_pointer_width = "64")]
impl Sub<usize> for VirtualAddress {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: usize) -> Self::Output {
        self - rhs as u64
    }
}

#[cfg(target_pointer_width = "64")]
impl SubAssign<usize> for VirtualAddress {
    #[inline]
    fn sub_assign(&mut self, rhs: usize) {
        self.sub_assign(rhs as u64)
    }
}

impl Sub<VirtualAddress> for VirtualAddress {
    type Output = u64;

    #[inline]
    fn sub(self, rhs: VirtualAddress) -> Self::Output {
        self.as_u64().checked_sub(rhs.as_u64()).unwrap()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PhysicalAddress(u64);

impl PhysicalAddress {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "x86_64")] {
            /// Create a new physical address.
            pub fn new(address: u64) -> Self {
                match address.get_bits(52..64) {
                    0 => Self(address),
                    _ => panic!("Invalid physical address"), // Invalid physical address as the bits 52..64 were not null.
                }
            }
        } else if #[cfg(target_arch = "aarch64")] {
            /// Create a new physical address.
            pub fn new(address: u64) -> Self {
                Self(address) // In aarch64 the bits 52..64 can be non-null.
            }
        }
    }

    #[inline(always)]
    pub fn align_down(self, alignment: u64) -> Self {
        Self(align_down(self.0, alignment))
    }

    #[inline(always)]
    pub fn is_aligned(self, alignment: u64) -> bool {
        self.align_down(alignment) == self
    }

    #[inline(always)]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl fmt::Debug for PhysicalAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("PhysicalAddress")
            .field(&format_args!("{:#x}", self.0))
            .finish()
    }
}

impl fmt::Binary for PhysicalAddress {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Binary::fmt(&self.0, f)
    }
}

impl fmt::LowerHex for PhysicalAddress {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

impl fmt::Octal for PhysicalAddress {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Octal::fmt(&self.0, f)
    }
}

impl fmt::UpperHex for PhysicalAddress {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::UpperHex::fmt(&self.0, f)
    }
}

impl fmt::Pointer for PhysicalAddress {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&(self.0 as *const ()), f)
    }
}

impl Add<u64> for PhysicalAddress {
    type Output = Self;

    #[inline]
    fn add(self, rhs: u64) -> Self::Output {
        PhysicalAddress::new(self.0 + rhs)
    }
}

impl AddAssign<u64> for PhysicalAddress {
    #[inline]
    fn add_assign(&mut self, rhs: u64) {
        *self = *self + rhs;
    }
}

#[cfg(target_pointer_width = "64")]
impl Add<usize> for PhysicalAddress {
    type Output = Self;

    #[inline]
    fn add(self, rhs: usize) -> Self::Output {
        self + rhs as u64
    }
}

#[cfg(target_pointer_width = "64")]
impl AddAssign<usize> for PhysicalAddress {
    #[inline]
    fn add_assign(&mut self, rhs: usize) {
        self.add_assign(rhs as u64)
    }
}

impl Sub<u64> for PhysicalAddress {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: u64) -> Self::Output {
        PhysicalAddress::new(self.0.checked_sub(rhs).unwrap())
    }
}

impl SubAssign<u64> for PhysicalAddress {
    #[inline]
    fn sub_assign(&mut self, rhs: u64) {
        *self = *self - rhs;
    }
}

#[cfg(target_pointer_width = "64")]
impl Sub<usize> for PhysicalAddress {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: usize) -> Self::Output {
        self - rhs as u64
    }
}

#[cfg(target_pointer_width = "64")]
impl SubAssign<usize> for PhysicalAddress {
    #[inline]
    fn sub_assign(&mut self, rhs: usize) {
        self.sub_assign(rhs as u64)
    }
}

impl Sub<PhysicalAddress> for PhysicalAddress {
    type Output = u64;
    #[inline]
    fn sub(self, rhs: PhysicalAddress) -> Self::Output {
        self.as_u64().checked_sub(rhs.as_u64()).unwrap()
    }
}

pub fn align_down(address: u64, align: u64) -> u64 {
    assert!(align.is_power_of_two(), "`align` must be a power of two");

    address & !(align - 1)
}

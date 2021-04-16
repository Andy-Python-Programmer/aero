use core::fmt;

use bit_field::BitField;

/// A canonical virtual memory address. The addresses are canonicalized based on
/// the target arch.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VirtualAddress(usize);

impl VirtualAddress {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "x86_64")] {
            /// Create a new canonical virtual memory address.
            pub fn new(address: usize) -> Self {
                match address.get_bits(47..64) {
                    0 | 0x1ffff => Self(address), // The address is canonical.
                    1 => Self(((address << 16) as isize >> 16) as usize), // The address needs to be truncated.
                    _ => panic!("Invalid virtual address"), // Invalid address as 48 to 64 do not have a valid sign extension.
                }
            }
        } else if #[cfg(target_arch = "aarch64")] {
            /// Create a new canonical virtual memory address.
            #[inline(always)]
            pub fn new(address: usize) -> Self {
                // On aarch64 there are there are no extra requirements.
                Self(address)
            }
        }
    }

    #[inline(always)]
    pub fn as_usize(&self) -> usize {
        self.0
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PhysicalAddres(usize);

impl PhysicalAddres {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "x86_64")] {
            /// Create a new physical address.
            pub fn new(address: usize) -> Self {
                match address.get_bits(52..64) {
                    0 => Self(address),
                    _ => panic!("Invalid physical address"), // Invalid physical address as the bits 52..64 were not null.
                }
            }
        } else if #[cfg(target_arch = "aarch64")] {
            /// Create a new physical address.
            pub fn new(address: usize) -> Self {
                Self(address) // In aarch64 the bits 52..64 can be non-null.
            }
        }
    }

    pub fn as_usize(&self) -> usize {
        self.0
    }
}

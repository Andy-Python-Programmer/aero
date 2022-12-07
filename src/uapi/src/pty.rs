use crate::ioctl;

pub const TIOCGPTN: usize = ioctl::ior::<u32>('T' as usize, 0x30);
pub const TIOCSPTLCK: usize = ioctl::iow::<i32>('T' as usize, 0x31);

use crate::ioctl;

pub const TIOCGPTN: usize = ioctl::ior::<u32>('T' as usize, 0x30);

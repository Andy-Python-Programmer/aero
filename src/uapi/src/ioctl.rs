/*
 * The generic ioctl numbering scheme doesn't really enforce
 * a type field. De facto, however, the top 8 bits of the lower 16
 * bits are indeed used as a type field, so we might just as well make
 * this explicit here. Please be sure to use the decoding functions
 * below from now on.
 */
pub const IOC_NRBITS: usize = 8;
pub const IOC_TYPEBITS: usize = 8;
pub const IOC_SIZEBITS: usize = 14;

pub const IOC_NRSHIFT: usize = 0;
pub const IOC_TYPESHIFT: usize = IOC_NRSHIFT + IOC_NRBITS;
pub const IOC_SIZESHIFT: usize = IOC_TYPESHIFT + IOC_TYPEBITS;
pub const IOC_DIRSHIFT: usize = IOC_SIZESHIFT + IOC_SIZEBITS;

pub const IOC_NONE: usize = 0;
pub const IOC_WRITE: usize = 1;
pub const IOC_READ: usize = 2;

pub const fn ioc(dir: usize, ty: usize, nr: usize, size: usize) -> usize {
    ((dir) << IOC_DIRSHIFT)
        | ((ty) << IOC_TYPESHIFT)
        | ((nr) << IOC_NRSHIFT)
        | ((size) << IOC_SIZESHIFT)
}

/*
 * Used to create numbers.
 *
 * NOTE: `iow` means userland is writing and kernel is reading. `ior`
 * means userland is reading and kernel is writing.
 */
#[inline]
pub const fn io(typ: usize, nr: usize) -> usize {
    ioc(IOC_NONE, typ, nr, 0)
}

#[inline]
pub const fn ior<T>(typ: usize, nr: usize) -> usize {
    ioc(IOC_READ, typ, nr, core::mem::size_of::<T>())
}

#[inline]
pub const fn iow<T>(typ: usize, nr: usize) -> usize {
    ioc(IOC_WRITE, typ, nr, core::mem::size_of::<T>())
}

#[inline]
pub const fn iowr<T>(typ: usize, nr: usize) -> usize {
    ioc(IOC_READ | IOC_WRITE, typ, nr, core::mem::size_of::<T>())
}

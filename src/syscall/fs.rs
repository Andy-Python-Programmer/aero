use super::SyscallResult;
use crate::println;

pub fn open(path: &[u8], flags: usize) -> SyscallResult<usize> {
    println!("Open {:?}: {:X}", core::str::from_utf8(path), flags);

    Ok(0)
}

/// Close a file.
pub fn close(fd: usize) -> SyscallResult<usize> {
    println!("Close: {}", fd);

    Ok(0)
}

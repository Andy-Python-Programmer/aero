//! A Virtual File System (VFS) is just an abstraction that many operating systems provide to applications.
//!
//! ## Notes
//! * https://wiki.osdev.org/VFS
//! * https://wiki.osdev.org/Hierarchical_VFS_Theory

use super::FileSystem;

pub struct AeroVfs {}

impl FileSystem for AeroVfs {
    const SIGNATURE: &'static str = "Aero VFS";
}

unsafe impl Send for AeroVfs {}
unsafe impl Sync for AeroVfs {}

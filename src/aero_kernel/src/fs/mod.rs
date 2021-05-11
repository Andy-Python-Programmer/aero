pub mod vfs;

/// File systems are the machine's way of ordering your data on readable and/or writable media.
///
/// ## Notes
/// * https://wiki.osdev.org/File_Systems
pub trait FileSystem: Send + Sync {
    const SIGNATURE: &'static str;
}

/// Initialize the file system. By default aero will use Aero VFS for its
/// file system.
pub fn init() {}

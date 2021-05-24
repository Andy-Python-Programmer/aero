use alloc::{collections::BTreeMap, sync::Arc};

use spin::RwLock;

pub mod devfs;
pub mod file_table;

static FILE_SYSTEMS: RwLock<BTreeMap<usize, Arc<dyn FileSystem>>> = RwLock::new(BTreeMap::new());

/// ## Notes
/// * https://wiki.osdev.org/File_Systems
pub trait FileSystem: Send + Sync {}

#[inline(always)]
pub(super) fn install_filesystem<F: 'static + FileSystem>(
    signature: usize,
    filesystem: F,
) -> Result<(), AeroFilesystemError> {
    let fs = FILE_SYSTEMS.read();

    if fs.contains_key(&signature) {
        Err(AeroFilesystemError::DeviceExists)
    } else {
        drop(fs);
        FILE_SYSTEMS.write().insert(signature, Arc::new(filesystem));

        Ok(())
    }
}

#[derive(Debug)]
pub enum AeroInvalidPath {
    /// If path's length is greater then `4096` (ie. The max path size in characters).
    HugePath,
}

#[derive(Debug)]
pub enum AeroFilesystemError {
    DeviceExists,
}

/// Structure that represents a path in a file system. This structure is a transparent
/// wrapper around ref [str].
#[repr(transparent)]
pub struct Path<'path>(&'path str);

impl<'path> Path<'path> {
    /// Aero uses `4096` as the maximum path size in characters.
    pub const MAX_PATH_SIZE: usize = 4096;

    pub fn new(path: &'path str) -> Result<Self, AeroInvalidPath> {
        /*
         * Make sure we do not accept any paths greater then the maximum path
         * size.
         */
        if path.len() > Self::MAX_PATH_SIZE {
            return Err(AeroInvalidPath::HugePath);
        }

        Ok(Self(path))
    }
}

pub fn init() {
    devfs::init().expect("Failed to initialize devfs");
}

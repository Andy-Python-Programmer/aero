use alloc::boxed::Box;

use hashbrown::HashMap;
use spin::Mutex;

pub mod devfs;

lazy_static::lazy_static! {
    pub static ref FILE_SYSTEMS: Mutex<HashMap<&'static str, Box<dyn FileSystem>>> = Mutex::new(HashMap::new());
}

/// ## Notes
/// * https://wiki.osdev.org/File_Systems
pub trait FileSystem: Send + Sync {}

#[inline(always)]
pub(super) fn install_filesystem<F: 'static + FileSystem>(
    signature: &'static str,
    filesystem: Box<F>,
) {
    FILE_SYSTEMS.lock().insert(signature, filesystem);
}

pub enum AeroInvalidPath {
    /// If path's length is greater then `4096` (ie. The max path size in characters).
    HugePath,
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
    devfs::init();
}

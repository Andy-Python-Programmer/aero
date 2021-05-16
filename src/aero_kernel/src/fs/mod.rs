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

pub fn init() {
    devfs::init();
}

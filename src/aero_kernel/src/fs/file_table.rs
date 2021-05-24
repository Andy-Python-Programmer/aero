use alloc::sync::Arc;
use alloc::vec::Vec;

use spin::RwLock;

pub struct FileHandle {
    fd: usize,
}

#[repr(transparent)]
pub struct FileTable(RwLock<Vec<Option<Arc<FileHandle>>>>);

impl FileTable {
    pub fn new() -> Self {
        Self(RwLock::new(Vec::new()))
    }

    pub fn get_handle(&self, fd: usize) -> Option<Arc<FileHandle>> {
        let files = self.0.read();

        if let Some(file) = &files.get(fd) {
            if let Some(handle) = file {
                return Some(handle.clone());
            }
        }

        None
    }
}

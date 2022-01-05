use alloc::sync::Arc;

use crate::fs::{
    inode::{FileType, INodeInterface, Metadata},
    Result,
};

pub struct UnixSocket;

impl UnixSocket {
    pub fn new() -> Arc<UnixSocket> {
        Arc::new(UnixSocket)
    }
}

impl INodeInterface for UnixSocket {
    fn metadata(&self) -> Result<Metadata> {
        Ok(Metadata {
            id: 0, // FIXME: What should this be?
            file_type: FileType::Socket,
            size: 0,
            children_len: 0,
        })
    }
}

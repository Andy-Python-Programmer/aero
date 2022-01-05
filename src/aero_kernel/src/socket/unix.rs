use alloc::sync::Arc;

use crate::fs::inode::INodeInterface;

pub struct UnixSocket;

impl UnixSocket {
    pub fn new() -> Arc<UnixSocket> {
        Arc::new(UnixSocket)
    }
}

impl INodeInterface for UnixSocket {}

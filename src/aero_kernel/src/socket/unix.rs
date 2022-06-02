/*
 * Copyright (C) 2021-2022 The Aero Project Developers.
 *
 * This file is part of The Aero Project.
 *
 * Aero is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Aero is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Aero. If not, see <https://www.gnu.org/licenses/>.
 */

use alloc::{
    string::String,
    sync::{Arc, Weak},
};

use crate::fs;
use crate::fs::{
    inode::{DirEntry, FileType, INodeInterface, Metadata},
    FileSystemError, Path, Result,
};

use super::SocketAddr;

pub struct UnixSocket {
    weak: Weak<UnixSocket>,
}

impl UnixSocket {
    pub fn new() -> Arc<UnixSocket> {
        Arc::new_cyclic(|weak| UnixSocket { weak: weak.clone() })
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

    fn bind(&self, address: SocketAddr, _length: usize) -> Result<()> {
        if let SocketAddr::Unix(address) = address {
            // The abstract namespace socket allows the creation of a socket
            // connection which does not require a path to be created.
            let abstrat_namespaced = address.path[0] == 0;
            assert!(!abstrat_namespaced);

            let path_len = address
                .path
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(address.path.len());

            let path_str = core::str::from_utf8(&address.path[..path_len])
                .ok()
                .ok_or(FileSystemError::InvalidPath)?;

            let path = Path::new(path_str);

            // Ensure that the path is not already in use.
            if fs::lookup_path(path).is_ok() {
                return Err(FileSystemError::EntryExists);
            }

            let (parent, name) = path.parent_and_basename();

            DirEntry::from_socket_inode(
                fs::lookup_path(parent)?,
                String::from(name),
                self.weak.upgrade().unwrap(),
            )?;

            Ok(())
        } else {
            Err(FileSystemError::NotSupported)
        }
    }

    fn connect(&self, _address: SocketAddr, _length: usize) -> Result<()> {
        log::error!("UnixSocket::connect() is not implemented");
        Ok(())
    }

    fn listen(&self, _backlog: usize) -> Result<()> {
        log::error!("UnixSocket::listen() not implemented");
        Ok(())
    }
}

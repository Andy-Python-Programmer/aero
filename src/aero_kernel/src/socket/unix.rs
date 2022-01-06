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

use aero_syscall::SocketAddr;
use alloc::{
    string::String,
    sync::{Arc, Weak},
};

use crate::fs;
use crate::fs::{
    inode::{DirEntry, FileType, INodeInterface, Metadata},
    FileSystemError, Path, Result,
};

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

    fn bind(&self, address: &SocketAddr, _length: usize) -> Result<()> {
        if let SocketAddr::Unix(address) = address {
            let path_len = address
                .path
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(address.path.len());

            let path_str = unsafe { core::str::from_utf8_unchecked(&address.path[0..path_len]) };
            let path = Path::new(path_str);

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
}

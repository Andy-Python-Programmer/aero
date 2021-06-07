/*
 * Copyright 2021 The Aero Project Developers. See the COPYRIGHT
 * file at the top-level directory of this project.
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 * <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
 * option. This file may not be copied, modified, or distributed
 * except according to those terms.
 */

use crate::utils::Downcastable;

use super::AeroFilesystemError;

/// Trait that represents an object which behaves like a file. For example device files,
/// files on the disk, etc...
///
/// This trait requires the implementor to implement [Send], [Sync] and [Downcastable] on
/// the inode structure.
pub trait INodeInterface: Send + Sync + Downcastable {
    /// Write at the provided `offset` with the given `buffer` as its contents.
    fn write_at(&self, offset: usize, buffer: &[u8]) -> Result<usize, AeroFilesystemError>;

    /// Read at the provided `offset` to the given `buffer.
    fn read_at(&self, offset: usize, buffer: &mut [u8]) -> Result<usize, AeroFilesystemError>;
}

/// An inode describes a file. An inode structure holds metadata of the
/// inode which includes its type, size, the number of links referring to it,
/// and the list of blocks holding the file's content.
#[derive(Clone)]
pub enum INode {}

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
extern crate alloc;

use core::marker::PhantomData;

use alloc::sync::Arc;
use hashbrown::HashMap;
use postcard::Deserializer;
use serde::Serialize;

pub mod async_runtime;

// private internals needed for the proc macro
pub mod __private {
    pub use alloc::sync::Arc;
    pub use alloc::vec::Vec;
    pub use spin::Mutex;
    pub use aero_syscall::sys_ipc_send;
}

pub fn serialize_buffer<T: Serialize>(t: T) -> Option<Vec<u8>> {
    match postcard::to_allocvec(&t) {
        Ok(data) => Some(data),
        Err(e) => {
            println!("serialize error: {}", e);
            None
        }
    }
}
pub fn deserialize_object<'a, T: serde::Deserialize<'a>>(data: &'a [u8]) -> Option<T> {
    let mut deser = deserializer(data);
    match T::deserialize(&mut deser) {
        Ok(value) => Some(value),
        Err(e) => {
            println!("deserialize error: {}", e);
            None
        }
    }
}
pub fn deserializer<'a>(data: &'a [u8]) -> Deserializer<'a> {
    Deserializer::from_bytes(data)
}
pub fn deserialize<'de, T: serde::Deserialize<'de>>(t: &mut Deserializer<'de>) -> Option<T> {
    match T::deserialize(t) {
        Ok(value) => Some(value),
        Err(e) => {
            println!("deserialize error: {}", e);
            None
        }
    }
}

pub struct ServerObject<T> {
    h: HashMap<usize, (/*owner*/ usize, Arc<Mutex<T>>)>,
    counter: usize,
    _ph: PhantomData<T>,
}
pub struct ClientObject {
    pub pid: usize,
    pub object_id: usize,
}
impl<T> ServerObject<T> {
    pub fn new() -> ServerObject<T> {
        ServerObject {
            h: HashMap::new(),
            counter: 0,
            _ph: PhantomData,
        }
    }
    pub fn do_drop(&mut self, src: usize, id: usize) {
        match self.h.get(&id) {
            Some((owner, _)) => {
                if *owner == src {
                    drop(owner);
                    self.h.remove(&id);
                } else {
                    println!(
                        "[aipc] pid {} tried to delete object {}, which is owned by {}",
                        src, id, *owner
                    );
                }
            }
            None => {
                println!(
                    "[aipc] pid {} tried to delete non-existent object {}",
                    src, id
                );
            }
        }
    }
    pub fn get(&mut self, src: usize, id: usize) -> Option<Arc<Mutex<T>>> {
        match self.h.get(&id) {
            Some((owner, data)) => {
                if *owner == src {
                    Some(Arc::clone(data))
                } else {
                    println!(
                        "[aipc] pid {} tried to operate on object {} owned by {}",
                        src, id, *owner
                    );
                    None
                }
            }
            None => {
                println!(
                    "[aipc] pid {} tried to operate on a non-existent object {}",
                    src, id
                );
                None
            }
        }
    }
    pub fn create(&mut self, src: usize, data: T) -> usize {
        let cntr = self.counter;
        self.counter += 1;
        self.h.insert(cntr, (src, Arc::new(Mutex::new(data))));
        return cntr;
    }
}
pub use aipc_proc::{def, object};
use spin::Mutex;

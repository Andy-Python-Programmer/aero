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

use core::sync::atomic::{AtomicU32, Ordering};

use aero_syscall::{SyscallError, TimeSpec};
use alloc::sync::Arc;
use spin::Once;

use crate::mem::paging::{PhysAddr, Translate, VirtAddr};
use crate::mem::AddressSpace;
use crate::userland::scheduler;
use crate::utils::sync::{BlockQueue, Mutex};

pub struct FutexContainer {
    futexes: Mutex<hashbrown::HashMap<PhysAddr, Arc<BlockQueue>>>,
}

impl FutexContainer {
    fn new() -> Self {
        Self {
            futexes: Mutex::new(hashbrown::HashMap::new()),
        }
    }

    /// Ensures the user-provided futex pointer is non-null and aligned to the alignment of
    /// a futex word (32-bits).
    fn validate_futex_ptr(ptr: VirtAddr) -> Result<(), SyscallError> {
        let raw = ptr.as_u64() as usize;

        if raw == 0 || (raw & (core::mem::size_of::<u32>() - 1)) == 0 {
            Err(SyscallError::EINVAL)
        } else {
            Ok(())
        }
    }

    /// Converts the user-provided futex word pointer to a unique futex key.
    fn addr_as_futex_key(ptr: VirtAddr) -> Option<PhysAddr> {
        let mut address_space = AddressSpace::this();
        let offset_table = address_space.offset_page_table();

        offset_table.translate_addr(ptr)
    }

    /// Returns the futex at the given key; allocating it if it doesn't exist.
    fn get_alloc(&self, key: PhysAddr) -> Arc<BlockQueue> {
        let mut container = self.futexes.lock();

        if let Some(futex) = container.get(&key) {
            futex.clone()
        } else {
            let futex = Arc::new(BlockQueue::new());
            container.insert(key, futex.clone());
            futex
        }
    }

    /// Returns the futex at the given key, or None if it doesn't exist.
    fn get(&self, key: PhysAddr) -> Option<Arc<BlockQueue>> {
        self.futexes.lock_irq().get(&key).map(|e| e.clone())
    }

    /// Tests the that the value at the futex word pointed to by `uaddr` still contains the
    /// `expected` value, and if so, it sleeps waiting for a futex wake operation on the
    /// futex word.
    fn wait(
        &self,
        uaddr: VirtAddr,
        expected: u32,
        _timeout: &TimeSpec,
    ) -> Result<(), SyscallError> {
        Self::validate_futex_ptr(uaddr)?;

        let key = Self::addr_as_futex_key(uaddr).ok_or(SyscallError::EINVAL)?;
        let value = uaddr.read_mut::<AtomicU32>().ok_or(SyscallError::EINVAL)?;

        if value.load(Ordering::SeqCst) == expected {
            let futex = self.get_alloc(key);

            let scheduler = scheduler::get_scheduler();
            let current_task = scheduler.current_task();

            futex.insert(current_task.clone());
            scheduler.inner.await_io()?;
            futex.remove(current_task);

            if futex.is_empty() {
                self.futexes.lock().remove(&key);
            }

            Ok(())
        } else {
            Err(SyscallError::EAGAIN)
        }
    }

    fn wake(&self, uaddr: VirtAddr) -> Result<(), SyscallError> {
        Self::validate_futex_ptr(uaddr)?;

        let key = Self::addr_as_futex_key(uaddr).ok_or(SyscallError::EINVAL)?;
        let futex = self.get(key).ok_or(SyscallError::EINVAL)?;

        futex.notify_complete();

        // todo: early reschedule if the futex is not empty.
        Ok(())
    }
}

static FUTEX_CONTAINER: Once<FutexContainer> = Once::new();

/// Returns a reference to the futex container; initializing if necessary.
fn get_futex_container() -> &'static FutexContainer {
    FUTEX_CONTAINER.call_once(|| FutexContainer::new())
}

#[syscall]
pub fn wait(ptr: usize, expected: usize, timeout: &TimeSpec) -> Result<usize, SyscallError> {
    let ptr = VirtAddr::new(ptr as u64);

    let futex_container = get_futex_container();
    futex_container.wait(ptr, expected as u32, timeout)?;

    Ok(0)
}

#[syscall]
pub fn wake(ptr: usize) -> Result<usize, SyscallError> {
    let ptr = VirtAddr::new(ptr as u64);

    let futex_container = get_futex_container();
    futex_container.wake(ptr)?;

    Ok(0)
}

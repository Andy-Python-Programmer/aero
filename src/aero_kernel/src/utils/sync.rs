// Copyright (C) 2021-2024 The Aero Project Developers.
//
// This file is part of The Aero Project.
//
// Aero is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Aero is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Aero. If not, see <https://www.gnu.org/licenses/>.

use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::arch::interrupts;
use crate::userland::scheduler;
use crate::userland::signals::SignalResult;
use crate::userland::task::Task;

/// Used to manage and block threads that are waiting for a condition to be true.
pub struct WaitQueue {
    queue: Mutex<Vec<Arc<Task>>>,
}

impl WaitQueue {
    /// Creates a new block queue.
    pub const fn new() -> Self {
        Self {
            queue: Mutex::new(Vec::new()),
        }
    }

    /// Run a future to completion on the current task. This function will block
    /// the caller until the given future has completed.
    pub fn block_on<'future, T, F: FnMut(&mut MutexGuard<T>) -> bool>(
        &self,
        mutex: &'future Mutex<T>,
        mut future: F,
    ) -> SignalResult<MutexGuard<'future, T>> {
        let mut lock = mutex.lock_irq();

        // Check if the future was already completed.
        if future(&mut lock) {
            return Ok(lock);
        }

        let scheduler = scheduler::get_scheduler();
        let task = scheduler.current_task();

        self.queue.lock_irq().push(task.clone());

        // Wait until the future is completed.
        while !future(&mut lock) {
            core::mem::drop(lock); // Drop the IRQ lock and await for IO to complete.
            scheduler.inner.await_io()?;

            // Re-acquire the lock.
            lock = mutex.lock_irq();
        }

        self.remove(&task);
        Ok(lock)
    }

    pub fn insert(&self, task: Arc<Task>) {
        self.queue.lock_irq().push(task);
    }

    pub fn remove(&self, task: &Task) {
        let mut tasks = self.queue.lock_irq();

        tasks
            .iter()
            .enumerate()
            .find_map(|(id, this)| {
                if this.pid() == task.pid() {
                    Some(id)
                } else {
                    None
                }
            })
            .map(|i| tasks.remove(i));
    }

    /// Wakes up all of the process in the wait queue.
    pub fn notify_all(&self) {
        let scheduler = scheduler::get_scheduler();
        let this = self.queue.lock_irq();

        for task in this.iter() {
            scheduler.inner.wake_up(task.clone());
        }
    }

    /// Wakes up only the first process in the wait queue.
    pub fn notify(&self) {
        let scheduler = scheduler::get_scheduler();
        let this: MutexGuard<Vec<Arc<Task>>> = self.queue.lock_irq();

        if let Some(task) = this.first() {
            scheduler.inner.wake_up(task.clone());
        }
    }

    pub fn is_empty(&self) -> bool {
        self.queue.lock_irq().is_empty()
    }
}

/// Helper guard structure used to lock interrupts. When dropped, interrupts
/// are enabled again. This is useful for volatile operations where we don't
/// want to be interrupted.
pub struct IrqGuard {
    locked: bool,
}

impl IrqGuard {
    /// Creates a new IRQ guard. See the [`IrqGuard`] documentation for more.
    pub fn new() -> Self {
        let locked = interrupts::is_enabled();

        unsafe {
            interrupts::disable_interrupts();
        }

        Self { locked }
    }
}

impl Drop for IrqGuard {
    /// Drops the IRQ guard, enabling interrupts again. See the [`IrqGuard`]
    /// documentation for more.
    fn drop(&mut self) {
        if self.locked {
            unsafe { interrupts::enable_interrupts() }
        }
    }
}

/// A blocking-based lock providing mutually exclusive access to the data.
pub struct BMutex<T: ?Sized> {
    wq: WaitQueue,
    spin: Mutex<T>,
}

impl<T> BMutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            wq: WaitQueue::new(),
            spin: Mutex::new(value),
        }
    }

    pub fn lock(&self) -> BMutexGuard<T> {
        let task = scheduler::get_scheduler().current_task();
        self.wq.insert(task.clone());

        loop {
            if let Some(guard) = self.spin.inner.try_lock() {
                self.wq.remove(&task);

                return BMutexGuard { guard, mutex: self };
            }

            let _ = scheduler::get_scheduler().inner.await_io();
        }
    }
}

pub struct BMutexGuard<'a, T: ?Sized + 'a> {
    guard: spin::MutexGuard<'a, T>,
    mutex: &'a BMutex<T>,
}

impl<T: ?Sized> core::ops::Deref for BMutexGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.guard.deref()
    }
}

impl<T: ?Sized> core::ops::DerefMut for BMutexGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        self.guard.deref_mut()
    }
}

impl<T: ?Sized> Drop for BMutexGuard<'_, T> {
    fn drop(&mut self) {
        self.mutex.wq.notify();
    }
}

/// A spin-based lock providing mutually exclusive access to data.
pub struct Mutex<T: ?Sized> {
    inner: spin::Mutex<T>,
}

impl<T> Mutex<T> {
    /// Creates a new [`Mutex`] wrapping the supplied data.
    pub const fn new(value: T) -> Self {
        Self {
            inner: spin::Mutex::new(value),
        }
    }

    /// Locks the [`Mutex`] and returns a guard that permits access to the inner data.
    ///
    /// The returned value may be dereferenced for data access and the lock will be dropped
    /// when the guard falls out of scope.
    pub fn lock(&self) -> MutexGuard<T> {
        MutexGuard {
            guard: core::mem::ManuallyDrop::new(self.inner.lock()),
            irq_lock: false,
        }
    }

    /// Locks the [`Mutex`] and returns a IRQ guard that permits access to the inner data and
    /// disables interrupts while the lock is held.
    ///
    /// The returned value may be dereferenced for data access and the lock will be dropped and
    /// interrupts will be re-enabled when the guard falls out of scope. Deadlocks occur if a thread
    /// tries to acquire a lock that will never become free. Thus, locking interrupts is useful for
    /// volatile operations where we might be interrupted.
    pub fn lock_irq(&self) -> MutexGuard<T> {
        let irq_lock = interrupts::is_enabled();

        unsafe {
            interrupts::disable_interrupts();
        }

        MutexGuard {
            guard: core::mem::ManuallyDrop::new(self.inner.lock()),
            irq_lock,
        }
    }

    /// Force unlock this [`Mutex`].
    ///
    /// # Safety
    ///
    /// This is *extremely* unsafe if the lock is not held by the current thread. However, this
    /// can be useful in some instances for exposing the lock to FFI that doesn't know how to deal
    /// with RAII.
    pub unsafe fn force_unlock(&self) {
        self.inner.force_unlock()
    }
}

pub struct MutexGuard<'a, T: ?Sized + 'a> {
    guard: core::mem::ManuallyDrop<spin::MutexGuard<'a, T>>,
    irq_lock: bool,
}

impl<T: ?Sized> core::ops::Deref for MutexGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &self.guard
    }
}

impl<T: ?Sized> core::ops::DerefMut for MutexGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        &mut self.guard
    }
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            core::mem::ManuallyDrop::drop(&mut self.guard);
        }

        if self.irq_lock {
            unsafe {
                interrupts::enable_interrupts();
            }
        }
    }
}

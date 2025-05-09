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

use aero_syscall::{OpenFlags, SyscallError};
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::arch::interrupts;
use crate::fs::FileSystemError;
use crate::userland::scheduler;
use crate::userland::signals::SignalError;
use crate::userland::task::Task;

bitflags::bitflags! {
    #[derive(Debug, Copy, Clone)]
    pub struct WaitQueueFlags: u32 {
        const DISABLE_IRQ = 1 << 1;
        const NON_BLOCK = 1 << 2;
    }
}

impl WaitQueueFlags {
    pub const fn is_nonblock(&self) -> bool {
        self.contains(WaitQueueFlags::NON_BLOCK)
    }
}

impl From<OpenFlags> for WaitQueueFlags {
    fn from(flags: OpenFlags) -> Self {
        let mut result = WaitQueueFlags::empty();
        if flags.contains(OpenFlags::O_NONBLOCK) {
            result.insert(WaitQueueFlags::NON_BLOCK);
        }
        result
    }
}

#[derive(Debug, Copy, Clone)]
pub enum WaitQueueError {
    Interrupted,
    WouldBlock,
}

impl From<WaitQueueError> for FileSystemError {
    fn from(err: WaitQueueError) -> Self {
        match err {
            WaitQueueError::Interrupted => FileSystemError::Interrupted,
            WaitQueueError::WouldBlock => FileSystemError::WouldBlock,
        }
    }
}

impl From<WaitQueueError> for SyscallError {
    fn from(err: WaitQueueError) -> Self {
        match err {
            WaitQueueError::Interrupted => SyscallError::EINTR,
            WaitQueueError::WouldBlock => SyscallError::EAGAIN,
        }
    }
}

/// Queue of tasks waiting for an event to occur.
pub struct WaitQueue {
    queue: Mutex<Vec<Arc<Task>>>,
}

impl WaitQueue {
    /// Create a new wait queue.
    pub const fn new() -> Self {
        Self {
            queue: Mutex::new(Vec::new()),
        }
    }

    fn _wait<'a, T, F>(
        &self,
        mutex: &'a Mutex<T>,
        mut cond: F,
        interruptable: bool,
        flags: WaitQueueFlags,
    ) -> Result<MutexGuard<'a, T>, WaitQueueError>
    where
        F: FnMut(&mut MutexGuard<T>) -> bool,
    {
        let acquire = || {
            if flags.contains(WaitQueueFlags::DISABLE_IRQ) {
                mutex.lock()
            } else {
                mutex.lock_irq()
            }
        };

        let mut lock = acquire();
        if cond(&mut lock) {
            // Condition is already satisfied.
            return Ok(lock);
        }

        if flags.is_nonblock() {
            return Err(WaitQueueError::WouldBlock);
        }

        let scheduler = scheduler::get_scheduler();
        let task = scheduler::current_thread();

        // If no IRQs was requested, the above lock would have disabled them so,
        // `lock_irq` is not required here.
        self.queue.lock().push(task.clone());

        while !cond(&mut lock) {
            drop(lock);

            match scheduler.await_io() {
                Ok(()) => lock = mutex.lock_irq(),
                Err(SignalError::Interrupted) if !interruptable => lock = acquire(),

                Err(SignalError::Interrupted) => {
                    self.remove(&task);
                    return Err(WaitQueueError::Interrupted);
                }
            }
        }

        self.remove(&task);
        Ok(lock)
    }

    /// Sleeps until a condition is met.
    ///
    /// Should be used when waiting for events such as completion of disk I/O. Any signals sent
    /// while waiting shall not be delivered until the condition is met and the wait is over.
    pub fn wait_uninterruptible<'a, T, F>(
        &self,
        flags: WaitQueueFlags,
        mutex: &'a Mutex<T>,
        cond: F,
    ) -> MutexGuard<'a, T>
    where
        F: FnMut(&mut MutexGuard<T>) -> bool,
    {
        unsafe {
            self._wait(mutex, cond, false, flags)
                // SAFETY: [`SignalError`] cannot occur on non-interruptible wait.
                .unwrap_unchecked()
        }
    }

    /// Sleeps until a condition is met.
    ///
    /// Should be used when waiting for events such as data being written to a pipe. Returns
    /// [`SignalError::Interrupted`] if the wait was interrupted by a signal.
    pub fn wait<'a, T, F: FnMut(&mut MutexGuard<T>) -> bool>(
        &self,
        flags: WaitQueueFlags,
        mutex: &'a Mutex<T>,
        cond: F,
    ) -> Result<MutexGuard<'a, T>, WaitQueueError> {
        self._wait(mutex, cond, true, flags)
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
            scheduler.wake_up(task.clone());
        }
    }

    /// Wakes up only the first process in the wait queue.
    pub fn notify(&self) {
        let scheduler = scheduler::get_scheduler();
        let this: MutexGuard<Vec<Arc<Task>>> = self.queue.lock_irq();

        if let Some(task) = this.first() {
            scheduler.wake_up(task.clone());
        }
    }

    pub fn is_empty(&self) -> bool {
        self.queue.lock_irq().is_empty()
    }
}

impl Default for WaitQueue {
    fn default() -> Self {
        Self::new()
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

            let _ = scheduler::get_scheduler().await_io();
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

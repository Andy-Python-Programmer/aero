use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::arch::interrupts;
use crate::userland::scheduler;
use crate::userland::task::Task;

pub struct BlockQueue {
    queue: Mutex<Vec<Arc<Task>>>,
}

impl BlockQueue {
    /// Creates a new block queue.
    #[inline]
    pub fn new() -> Self {
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
    ) -> MutexGuard<'future, T> {
        let mut lock = mutex.lock_irq();

        // Check if the future was already completed.
        if future(&mut lock) {
            return lock;
        }

        let scheduler = scheduler::get_scheduler();
        let task = scheduler.current_task();

        self.queue.lock_irq().push(task.clone());

        // Wait until the future is completed.
        while !future(&mut lock) {
            core::mem::drop(lock); // Drop the IRQ lock and await for IO to complete.
            scheduler.inner.await_io();

            // Re-acquire the lock.
            lock = mutex.lock_irq();
        }

        self.remove_task(task);
        lock
    }

    /// Inner helper function which removes a task from the block queue.
    fn remove_task(&self, task: Arc<Task>) {
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

    /// Notify's all of the tasks in the blocker's queue that the future has been
    /// completed.
    pub fn notify_complete(&self) {
        let scheduler = scheduler::get_scheduler();
        let this = self.queue.lock_irq();

        for task in this.iter() {
            scheduler.inner.wake_up(task.clone());
        }
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

/// A spin-based lock providing mutually exclusive access to data.
pub struct Mutex<T> {
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
    /// tries to acquire a lock that will never become free. Thus, locking interrupts is useful for volatile
    /// operations where we might be interrupted.
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

impl<'a, T: ?Sized> core::ops::Deref for MutexGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.guard.deref()
    }
}

impl<'a, T: ?Sized> core::ops::DerefMut for MutexGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        self.guard.deref_mut()
    }
}

impl<'a, T: ?Sized> Drop for MutexGuard<'a, T> {
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

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

// TODO: The reason why XFE did not run was because of the FXSleep function which uses select() to
// sleep.

use aero_syscall::prelude::{EPollEvent, EPollEventFlags};
use aero_syscall::SyscallError;

use alloc::sync::Arc;
use hashbrown::HashMap;

use crate::userland::scheduler;
use crate::utils::sync::Mutex;

use super::inode::{INodeInterface, PollTable};
use super::FileSystemError;

pub struct EPoll {
    events: Mutex<HashMap<usize, EPollEvent>>,
}

impl EPoll {
    // FIXME: The bitflags does not support bitwise or operations in const.
    const PRIVATE_BITS: EPollEventFlags = EPollEventFlags::from_bits_truncate(
        EPollEventFlags::WAKEUP.bits()
            | EPollEventFlags::ONESHOT.bits()
            | EPollEventFlags::ET.bits()
            | EPollEventFlags::EXCLUSIVE.bits(),
    );

    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            events: Mutex::new(HashMap::new()),
        })
    }

    /// Adds an event to the interest list.
    ///
    /// ## Errors
    /// * `EEXIST`: The event already exists at `fd`.
    pub fn add_event(&self, fd: usize, event: EPollEvent) -> Result<(), SyscallError> {
        let mut events = self.events.lock_irq();

        if events.get(&fd).is_some() {
            return Err(SyscallError::EEXIST);
        }

        events.insert(fd, event);
        Ok(())
    }

    /// Removes an event from the interest list.
    ///
    /// ## Errors
    /// * `ENOENT`: The event does not exist at `fd`.
    pub fn remove_event(&self, fd: usize) -> Result<(), SyscallError> {
        let mut events = self.events.lock_irq();

        if events.get(&fd).is_none() {
            return Err(SyscallError::ENOENT);
        }

        events.remove(&fd);
        Ok(())
    }

    /// Change the settings associated with file descriptor in the interest list to the
    /// new settings specified in event.
    ///
    /// ## Errors
    /// * `ENOENT`: The event does not exist at `fd`.
    pub fn update_event(&self, fd: usize, event: EPollEvent) -> Result<(), SyscallError> {
        let mut events = self.events.lock_irq();

        if !events.contains_key(&fd) {
            return Err(SyscallError::ENOENT);
        }

        events.insert(fd, event);
        Ok(())
    }

    /// Retrieves ready events, and delivers them to the caller-supplied event buffer and
    /// returns the number of ready events if the call was successful.
    ///
    /// ## Arguments
    ///
    /// * `events`: Used to return information from the ready list about file descriptors in the
    ///   interest list that have some events available.
    ///
    /// * `max_events`: Maximum number of events.
    ///
    /// * `timeout`: specifies the minimum number of milliseconds that epoll wait will block.
    ///   Specifying a timeout of `-1` will block indefinitely. While specifying a timeout of `0`
    ///   will return immediately even if there are available no events.
    ///
    ///
    /// ## Blocking
    /// Blocks the current task until either:
    ///
    /// * A file descriptor delivers an event.
    /// * The call is interrupted by a signal handler.
    /// * The timeout expires.
    pub fn wait(
        &self,
        ret_events: &mut [EPollEvent],
        max_events: usize,
        timeout: usize,
    ) -> Result<usize, FileSystemError> {
        let current_task = scheduler::get_scheduler().current_task();
        let file_table = &current_task.file_table;

        let mut table = self.events.lock();
        let mut n = 0;

        let mut fds = alloc::vec![];
        let mut poll_table = PollTable::default();

        // Iterate over all the registered events and check if they are ready.
        for (fd, epoll_event) in table.iter_mut() {
            if n == max_events {
                break;
            }

            ret_events[n].events = EPollEventFlags::empty();
            ret_events[n].data = unsafe { core::mem::zeroed() };

            let fd = file_table
                .get_handle(*fd)
                .ok_or(FileSystemError::NotSupported)?; // EINVAL

            let flags = epoll_event.events;
            let ready: EPollEventFlags = fd.inode().poll(None)?.into();

            // If the event mask does not contain any poll(2) events, the event
            // descriptor is disabled.
            if flags == Self::PRIVATE_BITS {
                continue;
            }

            if !(ready & flags).is_empty() {
                ret_events[n].events = ready & flags;
                ret_events[n].data = epoll_event.data;

                if flags.contains(EPollEventFlags::ONESHOT) {
                    // The `EPOLLONESHOT` bit that disables the descriptor when an event is
                    // received, until the next `EPOLL_CTL_MOD` will be issued.
                    epoll_event.events = Self::PRIVATE_BITS;
                }

                n += 1;
                continue;
            }

            // Not ready; add the event to the poll table.
            fd.inode().poll(Some(&mut poll_table))?;
            fds.push((fd, epoll_event, flags));
        }

        // If all events are ready, we can return now.
        if n > 0 || fds.is_empty() {
            debug_assert!(fds.is_empty());
            return Ok(n);
        }

        // Start the timer if timeout specified, if not, we can block indefinitely.
        // If the timeout is zero, then we have to return without blocking.
        if timeout == 0 {
            return Ok(0);
        }

        if timeout > 0 {
            scheduler::get_scheduler()
                .inner
                .sleep(Some(timeout * 1_000_000))?;
        } else {
            scheduler::get_scheduler().inner.sleep(None)?;
        }

        'search: loop {
            scheduler::get_scheduler().inner.await_io()?;

            if current_task.load_sleep_duration() == 0 && timeout > 0 {
                // Timeout has expired.
                return Ok(0);
            }

            for (fd, event, flags) in fds.iter_mut() {
                // If the event mask does not contain any poll(2) events, the event
                // descriptor is disabled.
                if *flags == Self::PRIVATE_BITS {
                    continue;
                }

                let ready: EPollEventFlags = fd.inode().poll(None)?.into();

                if !(ready & *flags).is_empty() {
                    // The event is ready; break out of the search loop and set ready
                    // events to 1.
                    ret_events[n].events = ready & *flags;
                    ret_events[n].data = event.data;

                    if flags.contains(EPollEventFlags::ONESHOT) {
                        // The `EPOLLONESHOT` bit that disables the descriptor when an event is
                        // received, until the next `EPOLL_CTL_MOD` will be issued.
                        event.events = Self::PRIVATE_BITS;
                    }

                    n = 1;
                    break 'search;
                }
            }
        }

        Ok(n)
    }
}

unsafe impl Send for EPoll {}
unsafe impl Sync for EPoll {}

impl INodeInterface for EPoll {}

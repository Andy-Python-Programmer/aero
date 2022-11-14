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
    ///             interest list that have some events available.
    ///
    /// * `max_events`: Maximum number of events.
    ///
    /// * `timeout`: specifies the minimum number of milliseconds that epoll wait will block. Specifying
    ///              a timeout of `-1` will block indefinitely. While specifing a timeout of `0` will return
    ///              immediately even if there are available no events.
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
        ret_events: &mut [&mut EPollEvent],
        max_events: usize,
        _timeout: usize,
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

            let fd = file_table
                .get_handle(*fd)
                .ok_or(FileSystemError::NotSupported)?; // EINVAL

            let flags = epoll_event.events;
            let ready: EPollEventFlags = fd.inode().poll(None)?.into();

            if ready.contains(flags) {
                // The registered event is ready; increment the number of ready events
                // and set event flags mask for this event in the caller-supplied event
                // buffer.
                ret_events[n].events = ready & flags;
                n += 1;
                continue;
            }

            // Not ready; add the event to the poll table.
            fd.inode().poll(Some(&mut poll_table))?;
            fds.push(fd);
        }

        // If all events are ready, we can return now.
        if n > 0 {
            debug_assert!(fds.len() == 0);
            return Ok(n);
        }

        'search: loop {
            // Wait till one of the file descriptor deliever an event.
            scheduler::get_scheduler().inner.await_io()?;

            for fd in fds.iter() {
                let ready: EPollEventFlags = fd.inode().poll(None)?.into();
                let flags = table
                    .get(&fd.fd)
                    .ok_or(FileSystemError::NotSupported)?
                    .events;

                if ready.contains(flags) {
                    // The event is ready; break out of the search loop and set ready
                    // events to 1.
                    ret_events[0].events = ready & flags;
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

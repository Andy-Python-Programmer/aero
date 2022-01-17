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

use aero_syscall::signal::*;

use alloc::sync::Arc;

use core::ops::{Index, IndexMut};
use core::sync::atomic::{AtomicU64, Ordering};

use bit_field::BitField;

use aero_syscall::AeroSyscallError;

use super::scheduler;
use crate::fs::FileSystemError;
use crate::utils::sync::{Mutex, MutexGuard};

mod default {
    use crate::userland::scheduler;

    #[derive(Copy, Clone, PartialEq)]
    pub enum Action {
        Ignore,
        Handle(fn()),
    }

    /// Some of the default actions for the signals.
    static DEFAULT_ACTIONS: [Action; super::SIGNAL_COUNT] = [
        Action::Ignore,                   // UNUSED
        Action::Handle(terminate),        // SIGHUP
        Action::Handle(terminate),        // SIGINT
        Action::Handle(terminate),        // SIGQUIT
        Action::Handle(terminate),        // SIGILL
        Action::Ignore,                   // UNUSED
        Action::Ignore,                   // UNUSED
        Action::Handle(terminate),        // SIGBUS
        Action::Handle(terminate),        // SIGFPE
        Action::Handle(terminate),        // SIGKILL
        Action::Ignore,                   // UNUSED
        Action::Handle(terminate),        // SIGSEGV
        Action::Ignore,                   // UNUSED
        Action::Handle(terminate),        // SIGPIPE
        Action::Ignore,                   // UNUSED
        Action::Handle(terminate),        // SIGTERM
        Action::Ignore,                   // UNUSED
        Action::Ignore,                   // SIGCHLD
        Action::Handle(terminate),        // SIGCONT
        Action::Handle(stop),             // SIGSTOP
        Action::Handle(stop),             // SIGTSTP
        Action::Ignore,                   // UNUSED
        Action::Ignore,                   // UNUSED
        Action::Ignore,                   // UNUSED
        Action::Ignore,                   // UNUSED
        Action::Ignore,                   // UNUSED
        Action::Ignore,                   // UNUSED
        Action::Ignore,                   // UNUSED
        Action::Ignore,                   // UNUSED
        Action::Ignore,                   // UNUSED
        Action::Ignore,                   // UNUSED
        Action::Ignore,                   // UNUSED
        Action::Handle(terminate_thread), // UNUSED
    ];

    fn terminate() {
        scheduler::get_scheduler().inner.exit(1);
    }

    fn terminate_thread() {
        unimplemented!()
    }

    fn stop() {
        unimplemented!()
    }

    /// Get the default action for the provided `signal`.
    pub fn action(signal: usize) -> Action {
        DEFAULT_ACTIONS[signal]
    }

    /// Runs the default action for the provided `signal`.
    pub fn handle_default(signal: usize) {
        let action = DEFAULT_ACTIONS[signal];

        if let Action::Handle(f) = action {
            (f)();
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum SignalError {
    Interrupted,
}

const IMMUTABLE_MASK: u64 = (1u64 << SIGSTOP) | (1u64 << SIGCONT) | (1u64 << SIGKILL);

/// Returns [`true`] if the provided `signal` is overridable.
fn can_override(signal: usize) -> bool {
    IMMUTABLE_MASK.get_bit(signal) == false
}

pub type SignalResult<T> = core::result::Result<T, SignalError>;

impl From<SignalError> for FileSystemError {
    fn from(s: SignalError) -> Self {
        match s {
            SignalError::Interrupted => FileSystemError::Interrupted,
        }
    }
}

impl From<SignalError> for AeroSyscallError {
    fn from(s: SignalError) -> Self {
        match s {
            SignalError::Interrupted => AeroSyscallError::EINTR,
        }
    }
}

#[derive(Default, Copy, Clone, Debug)]
pub struct SignalEntry {
    handler: SignalHandler,
    flags: SignalFlags,
    mask: u64,
    sigreturn: usize,
}

impl SignalEntry {
    /// Create a new `SignalEntry` with the provided `sigaction` and `sigreturn`.
    pub fn from_sigaction(
        sigaction: SigAction,
        sigreturn: usize,
    ) -> Result<SignalEntry, AeroSyscallError> {
        Ok(SignalEntry {
            handler: SignalHandler::from(sigaction.sa_handler),
            flags: SignalFlags::from_bits(sigaction.sa_flags).ok_or(AeroSyscallError::EINVAL)?,
            mask: sigaction.sa_mask,
            sigreturn,
        })
    }

    pub fn into_sigaction(&self) -> SigAction {
        let handler: usize = self.handler.into();

        SigAction {
            sa_handler: handler as u64,
            sa_mask: self.mask,
            sa_flags: self.flags.bits(),
            sa_sigaction: 0,
        }
    }
}

impl SignalEntry {
    /// Returns the `SignalHandler` for this `SignalEntry`.
    pub fn handler(&self) -> SignalHandler {
        self.handler
    }

    pub fn sigreturn(&self) -> usize {
        self.sigreturn
    }
}

const SIGNAL_COUNT: usize = 33;

#[derive(Copy, Clone)]
pub struct Entries {
    entries: [SignalEntry; SIGNAL_COUNT],
    pending_mask: u64,
}

impl Default for Entries {
    fn default() -> Entries {
        Entries {
            entries: [SignalEntry::default(); SIGNAL_COUNT],
            pending_mask: 0,
        }
    }
}

impl Index<usize> for Entries {
    type Output = SignalEntry;

    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[index]
    }
}

impl IndexMut<usize> for Entries {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.entries[index]
    }
}

impl Entries {
    /// Returns the pending mask.
    pub fn pending(&self) -> u64 {
        self.pending_mask
    }

    /// Marks the provided `signal` as not pending.
    pub fn clear_pending(&mut self, signal: u64) {
        self.pending_mask.set_bit(signal as usize, false);
    }

    /// Sets the provided `signal` to be pending.
    pub fn set_pending(&mut self, signal: u64) {
        self.pending_mask.set_bit(signal as usize, true);
    }
}

pub struct Signals {
    entries: Arc<Mutex<Entries>>,
    blocked_mask: AtomicU64,
    thread_pending_mask: AtomicU64,
}

impl Signals {
    pub(super) fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(Default::default())),
            blocked_mask: AtomicU64::new(0),
            thread_pending_mask: AtomicU64::new(0),
        }
    }
}

impl Clone for Signals {
    fn clone(&self) -> Self {
        Signals {
            entries: self.entries.clone(),
            blocked_mask: AtomicU64::new(self.blocked_mask.load(Ordering::SeqCst)),
            thread_pending_mask: AtomicU64::new(0),
        }
    }
}

pub enum TriggerResult {
    Ignored,
    Blocked,
    Triggered,
}

impl Signals {
    pub fn entries(&self) -> MutexGuard<Entries> {
        self.entries.lock_irq()
    }

    pub fn thread_pending(&self) -> u64 {
        self.thread_pending_mask.load(Ordering::SeqCst)
    }

    pub fn pending(&self) -> u64 {
        self.thread_pending() | self.entries().pending()
    }

    pub fn is_pending(&self, signal: u64) -> bool {
        self.pending().get_bit(signal as usize)
    }

    pub fn clear_pending(&self, signal: u64) {
        if self.thread_pending().get_bit(signal as usize) {
            self.thread_pending_mask
                .fetch_and(!(1u64 << signal), Ordering::SeqCst);
        } else {
            self.entries().clear_pending(signal);
        }
    }

    pub fn set_pending(&self, signal: u64, thread_scope: bool) {
        if thread_scope {
            self.thread_pending_mask
                .fetch_or(1u64 << signal, Ordering::SeqCst);
        } else {
            self.entries().set_pending(signal);
        }
    }

    /// Returns [`true`] if has pending signals.
    pub fn has_pending(&self) -> bool {
        (self.entries().pending() | self.thread_pending()) & !self.blocked_mask() > 0
    }

    /// Returns the blocked mask.
    pub fn blocked_mask(&self) -> u64 {
        self.blocked_mask.load(Ordering::SeqCst)
    }

    /// Returns [`true`] if the provided `signal` is blocked.
    pub fn is_blocked(&self, signal: usize) -> bool {
        self.blocked_mask().get_bit(signal)
    }

    pub fn trigger(&self, signal: usize, this_thread: bool) -> TriggerResult {
        assert!(signal < SIGNAL_COUNT);

        let sigs = self.entries();
        let handler = sigs[signal].handler();

        if match handler {
            SignalHandler::Ignore => false,

            SignalHandler::Default => {
                let action = default::action(signal);

                match action {
                    default::Action::Ignore => false,
                    default::Action::Handle(_) => true,
                }
            }

            SignalHandler::Handle(_) => true,
        } {
            core::mem::drop(sigs); // drop the lock
            self.set_pending(signal as u64, this_thread);

            if self.is_blocked(signal) {
                TriggerResult::Blocked
            } else {
                TriggerResult::Triggered
            }
        } else {
            TriggerResult::Ignored
        }
    }

    /// Clear the signal entries and blocked mask.
    pub fn clear(&self) {
        *self.entries.lock_irq() = Entries::default();
        self.blocked_mask.store(0, Ordering::SeqCst);
    }

    pub fn set_signal(
        &self,
        signal: usize,
        handler: Option<SignalEntry>,
        old: Option<&mut SigAction>,
    ) {
        assert!(signal < SIGNAL_COUNT);

        if !can_override(signal) {
            return;
        }

        let mut signals = self.entries();

        if let Some(old) = old {
            *old = signals[signal].into_sigaction();
        }

        if let Some(handler) = handler {
            signals[signal] = handler;
        }
    }

    /// Copy over the signals from the provided `signals`.
    pub fn copy_from(&self, signals: &Signals) {
        // Copy over the signl entries.
        *self.entries() = *signals.entries();

        // Copy over the blocked mask.
        self.blocked_mask.store(
            signals.blocked_mask.load(Ordering::SeqCst),
            Ordering::SeqCst,
        );
    }

    pub fn set_mask(&self, how: SigProcMask, set: u64, old_set: Option<&mut u64>) {
        if let Some(old) = old_set {
            *old = self.blocked_mask.load(Ordering::SeqCst);
        }

        let set = set & !IMMUTABLE_MASK;

        match how {
            SigProcMask::Block => {
                self.blocked_mask.fetch_or(set, Ordering::SeqCst);
            }

            SigProcMask::Unblock => {
                self.blocked_mask.fetch_and(!set, Ordering::SeqCst);
            }

            SigProcMask::Set => {
                self.blocked_mask.store(set, Ordering::SeqCst);
            }
        }
    }
}

pub fn check_for_signals() -> Option<(usize, SignalEntry)> {
    let task = scheduler::get_scheduler().current_task();
    let signals = task.signals();

    // Check if there are any pending signals.
    if !signals.has_pending() {
        return None;
    }

    // Check if a SIGKILL is pending, and if so, kill the task.
    if signals.is_pending(SIGKILL as u64) {
        signals.clear_pending(SIGKILL as u64);
        scheduler::get_scheduler().inner.exit(1);
    }

    for i in 0..SIGNAL_COUNT {
        if !signals.is_blocked(i) && signals.is_pending(i as u64) {
            signals.clear_pending(i as u64);

            let entries = signals.entries();
            let entry = entries[i];

            match entry.handler() {
                SignalHandler::Default => {
                    drop(entries);
                    default::handle_default(i);
                }

                SignalHandler::Handle(_) => {
                    return Some((i, entry));
                }

                SignalHandler::Ignore => {
                    unreachable!()
                }
            }
        }
    }

    None
}

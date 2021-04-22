use crate::userland::elf::Elf;
use core::sync::atomic::{AtomicUsize, Ordering};

pub static PID_COUNTER: PIDCounter = PIDCounter::new();

#[derive(Debug)]
pub struct PIDCounter(AtomicUsize);

impl PIDCounter {
    pub const fn new() -> Self {
        Self(AtomicUsize::new(1))
    }

    pub fn next(&self) -> usize {
        self.0.fetch_add(1, Ordering::AcqRel)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ProcessState {
    Running,
    Dead,
}

#[derive(Debug)]
pub struct Process {
    pub pid: usize,
    pub pc: usize,
    pub state: ProcessState,
}

impl Process {
    pub fn new(binary: &Elf) -> Self {
        Self {
            pid: PID_COUNTER.next(),
            pc: binary.header.e_entry as usize,
            state: ProcessState::Running,
        }
    }

    pub fn from_function(function: unsafe extern "C" fn()) -> Self {
        Self {
            pid: PID_COUNTER.next(),
            pc: function as usize,
            state: ProcessState::Running,
        }
    }
}

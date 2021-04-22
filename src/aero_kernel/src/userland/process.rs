use core::sync::atomic::{AtomicUsize, Ordering};

use x86_64::VirtAddr;

use xmas_elf::{
    header,
    program::{self, Type},
    ElfFile,
};

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
    pub entry_point: VirtAddr,
    pub state: ProcessState,
}

impl Process {
    pub fn new(binary: &ElfFile) -> Self {
        header::sanity_check(binary).expect("The binary failed the sanity check");

        let entry_point = VirtAddr::new(binary.header.pt2.entry_point());

        for header in binary.program_iter() {
            program::sanity_check(header, binary).expect("Failed header sanity check");

            let header_type = header.get_type().expect("Unable to get the header type");

            if let Type::Load = header_type {}
        }

        let this = Self {
            pid: PID_COUNTER.next(),
            entry_point,
            state: ProcessState::Running,
        };

        this
    }
}

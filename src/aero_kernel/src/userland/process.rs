use core::sync::atomic::{AtomicUsize, Ordering};

use x86_64::VirtAddr;

use xmas_elf::{
    header,
    program::{self, Type},
    ElfFile,
};

/// The process id counter. Increment after a new process is created.
static PID_COUNTER: PIDCounter = PIDCounter::new();

#[derive(Debug)]
#[repr(transparent)]
struct PIDCounter(AtomicUsize);

impl PIDCounter {
    /// Create a new process id counter.
    #[inline(always)]
    const fn new() -> Self {
        Self(AtomicUsize::new(1))
    }

    /// Increment the process id by 1.
    #[inline(always)]
    fn next(&self) -> usize {
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
    /// Create a new process from an [ElfFile].
    pub fn from_elf(binary: &ElfFile) -> Self {
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

    /// Create a new process from a function.
    ///
    /// ## Notes
    ///
    /// Make sure the function has the `#[naked]` attribute. It does **not**
    /// matter if the function's name is mangled.
    pub fn from_function(function: unsafe extern "C" fn()) -> Self {
        let this = Self {
            pid: PID_COUNTER.next(),
            entry_point: VirtAddr::new((&function as *const _) as u64),
            state: ProcessState::Running,
        };

        this
    }
}

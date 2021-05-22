use x86_64::VirtAddr;

#[repr(C)]
pub(super) struct Context {
    pub cr3: u64,
    pub rsp: u64,
    pub rflags: u64,
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub rip: u64,
}

impl Context {
    pub fn new() -> Self {
        Self {
            cr3: 0x00,
            rsp: 0x00,
            rflags: 0x00,
            r15: 0x00,
            r14: 0x00,
            r13: 0x00,
            r12: 0x00,
            rbp: 0x00,
            rbx: 0x00,
            rip: 0x00,
        }
    }

    pub fn set_stack_top(&mut self, stack_top: VirtAddr) {
        self.rsp = stack_top.as_u64();
    }

    pub fn set_instruction_ptr(&mut self, func: VirtAddr) {
        self.rip = func.as_u64();
    }

    pub fn get_stack_top(&self) -> VirtAddr {
        unsafe { VirtAddr::new_unsafe(self.rsp) }
    }

    pub fn get_instruction_ptr(&self) -> VirtAddr {
        unsafe { VirtAddr::new_unsafe(self.rip) }
    }
}

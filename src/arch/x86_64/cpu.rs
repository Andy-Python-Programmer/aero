pub struct CPUState {
    pub ax: usize,
    pub bx: usize,
    pub cx: usize,
    pub dx: usize,
}

impl CPUState {
    pub fn new() -> Self {
        let ax;
        let bx;
        let cx;
        let dx;

        unsafe {
            asm!("mov {}, rax", out(reg) ax);
            asm!("mov {}, rbx", out(reg) bx);
            asm!("mov {}, rcx", out(reg) cx);
            asm!("mov {}, rdx", out(reg) dx);
        }

        Self { ax, bx, cx, dx }
    }
}

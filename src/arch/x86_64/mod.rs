pub mod cpu;
pub mod gdt;
pub mod interrupts;
pub mod memory;
pub mod elf {
    pub use goblin::elf64::*;
}

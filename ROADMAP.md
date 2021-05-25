## Roadmap May - June 2021
- [x] Load a shell as a seperate program and run it
- [x] `sysret` vulnerability
- [x] Refactors
- [x] Higher Half kernel
- [x] Minimal standard library
- [x] Thread Local Storage (Per-Cpu storage)
- [x] Improved logger
- [x] Store logs in ring buffer
- [x] Devfs
- [ ] Filesystem table isolated for each process
- [ ] Re-implement the APIC trampoline in more clean manner
- [ ] Kernel drivers

## Roadmap April - May 2021
- [x] Task State Segment
- [x] New Aero UEFI bootloader
- [x] Port the kernel to the new UEFI bootloader
- [x] Initialize the local APIC
- [x] Port the VGA driver to use the framebuffer provided by the bootloader instead
- [x] Enter ring 3
- [x] ELF file parser
- [x] Load the kernel as an ELF file
- [x] 64-bit `syscall` instruction handler
- [x] Basic Task Scheduler
- [x] Exit syscall 
- [x] IO Apic
- [x] MADT table - Unlock the full potential of the CPU(s)


## Roadmap March - April 2021

- [x] Global Descriptor Table
- [x] Interrupt Descriptor Table
- [x] Programmable Interval Timer
- [x] Paging
- [x] Mouse Interrupts
- [x] Keyboard Interrupts
- [x] ACPI Tables
- [x] Syscalls Handler
- [x] PCI Driver
- [x] SATA Drive Detect

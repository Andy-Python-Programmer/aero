macro define_syscall_fns($(pub fn $sys_fn:ident($a:ident $(,$b:ident $(,$c:ident $(,$d:ident $(,$e:ident $(,$f:ident)?)?)?)?)?) -> usize;)+) {
    $(
        pub unsafe fn $sys_fn(mut $a: usize, $($b: usize, $($c: usize, $($d: usize, $($e: usize, $($f: usize)?)?)?)?)?) -> usize {
            if $crate::syscall::supports_syscall_sysret() {
                asm!(
                    "syscall",
                    inout("rax") $a,
                    $(in("rdi") $b, $(in("rsi") $c, $(in("rdx") $d, $(in("r10") $e, $(in("r8") $f,)?)?)?)?)?
                    out("rcx") _,
                    out("r11") _,
                    options(nostack),
                );
            } else {
                asm!(
                    "int 0x80",
                    inout("rax") $a,
                    $(in("rdi") $b, $(in("rsi") $c, $(in("rdx") $d, $(in("r10") $e, $(in("r8") $f,)?)?)?)?)?
                    out("rcx") _,
                    out("r11") _,
                    options(nostack),
                );
            }

            $a
        }
    )+
}

define_syscall_fns!(
    pub fn syscall0(a) -> usize;
    pub fn syscall1(a, b) -> usize;
    pub fn syscall2(a, b, c) -> usize;
    pub fn syscall3(a, b, c, d) -> usize;
    pub fn syscall4(a, b, c, d, e) -> usize;
    pub fn syscall5(a, b, c, d, e, f) -> usize;
);

/// Returns true if the current CPU supports the `syscall` and
/// the `sysret` instruction.
#[inline(always)]
pub fn supports_syscall_sysret() -> bool {
    #[cfg(target_pointer_width = "64")]
    {
        true
    }

    #[cfg(not(target_pointer_width = "64"))]
    {
        false
    }
}

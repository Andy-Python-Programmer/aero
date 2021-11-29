macro define_syscall_fns($(pub fn $sys_fn:ident($a:ident $(,$b:ident $(,$c:ident $(,$d:ident $(,$e:ident $(,$f:ident $(,$g:ident)?)?)?)?)?)?) -> usize;)+) {
    $(
        pub fn $sys_fn(mut $a: usize, $($b: usize, $($c: usize, $($d: usize, $($e: usize, $($f: usize, $($g: usize)?)?)?)?)?)?) -> usize {
            unsafe {
                    asm!(
                        "syscall",
                        inout("rax") $a,
                        $(in("rdi") $b, $(in("rsi") $c, $(in("rdx") $d, $(in("r10") $e, $(in("r8") $f, $(in("r9") $g,)?)?)?)?)?)?
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
    pub fn syscall6(a, b, c, d, e, f, g) -> usize;
);

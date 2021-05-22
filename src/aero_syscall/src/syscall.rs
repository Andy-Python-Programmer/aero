use raw_cpuid::CpuId;
use spin::Once;

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
    cfg_if::cfg_if!(
        if #[cfg(target_pointer_width = "64")] {
            static CACHE: Once<bool> = Once::new(); // Cache the result of `cpuid` feature info.

            return *CACHE.call_once(|| {
                let function_info = CpuId::new()
                    .get_extended_function_info()
                    .expect("Failed to retrieve CPU function info");

                function_info.has_syscall_sysret()
            });
        } else {
            /*
             * We are sure if the target pointer width is not 64 then syscall instruction is
             * not supported.
             */
            return false;
        }
    );
}

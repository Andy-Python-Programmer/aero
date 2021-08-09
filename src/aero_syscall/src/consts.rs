macro define_syscall_ids($($name:ident => $id:expr;)*) {
    $(
        pub const $name: usize = $id;
    )*
}

define_syscall_ids!(
    SYS_READ => 0;
    SYS_WRITE => 1;
    SYS_OPEN => 2;
    SYS_CLOSE => 3;
    SYS_SHUTDOWN => 10;
    SYS_EXIT => 60;
    SYS_FORK => 61;
    SYS_REBOOT => 169;
);

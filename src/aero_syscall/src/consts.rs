macro define_syscall_ids($($name:ident => $id:expr;)*) {
    $(
        pub const $name: usize = $id;
    )*
}

define_syscall_ids!(
    SYS_WRITE => 1;
    SYS_EXIT => 60;
    SYS_REBOOT => 169;
);

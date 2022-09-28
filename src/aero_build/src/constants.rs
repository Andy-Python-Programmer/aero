use std::path::Path;

#[macro_export]
macro_rules! const_str {
    ($name:ident, $value:expr) => {
        pub const $name: &'static str = $value;
    };
}

const_str!(OVMF_URL, "https://github.com/aero-os/ovmf-prebuilt");

// pub const OVMF_URL = "https://github.com/aero-os/ovmf-prebuilt";
const_str!(LIMINE_URL, "https://github.com/limine-bootloader/limine");

const_str!(BUILD_DIR, "build");
const_str!(BUNDLED_DIR, "bundled");
const_str!(SYSROOT_DIR, "sysroot");
const_str!(SYSROOT_CARGO_HOME, "sysroot/cargo-home");
const_str!(EXTRA_FILES, "extra-files");
const_str!(BASE_FILES_DIR, "base-files");
const_str!(LIMINE_TEMPLATE, r#"
TIMEOUT=0
VERBOSE=yes

:aero
PROTOCOL=limine
KASLR=no
KERNEL_PATH=boot:///aero.elf
CMDLINE=term-background=background theme-background=0x50000000

MODULE_PATH=boot:///term_background.bmp
MODULE_CMDLINE=background

MODULE_PATH=boot:///initramfs.cpio
MODULE_CMDLINE=initramfs
"#);

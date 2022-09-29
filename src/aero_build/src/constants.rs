pub const OVMF_URL: &'static str = "https://github.com/aero-os/ovmf-prebuilt";
pub const LIMINE_URL: &'static str = "https://github.com/limine-bootloader/limine";

pub const BUILD_DIR: &'static str = "build";
pub const BUNDLED_DIR: &'static str = "bundled";
pub const SYSROOT_DIR: &'static str = "sysroot";
pub const SYSROOT_CARGO_HOME: &'static str = "sysroot/cargo-home";
pub const EXTRA_FILES: &'static str = "extra-files";
pub const BASE_FILES_DIR: &'static str = "base-files";
pub const LIMINE_TEMPLATE: &'static str = r#"
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
"#;

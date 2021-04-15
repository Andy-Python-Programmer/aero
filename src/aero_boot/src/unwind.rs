use core::panic::PanicInfo;

#[panic_handler]
pub unsafe extern "C" fn rust_begin_unwind(info: &PanicInfo) -> ! {
    log::error!("{}", info);

    asm!("cli");

    loop {
        asm!("hlt");
    }
}

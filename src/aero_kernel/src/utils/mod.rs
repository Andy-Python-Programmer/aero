pub mod buffer;
pub mod io;
pub mod linker;
pub mod stack;

/// Push scratch registers.
pub macro push_scratch() {
    "
    /*
    * Push scratch registers.
    */

    push rcx
    push rdx
    push rdi
    push rsi
    push r8
    push r9
    push r10
    push r11
    "
}

/// Push preserved registers.
pub macro push_preserved() {
    "
    /*
    * Push preserved registers.
    */

    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    "
}

pub macro pop_preserved() {
    "
    /* 
    * Pop preserved registers.
    */

    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    "
}

/// Pop scratch registers.
pub macro pop_scratch() {
    "
    /* 
    * Pop scratch registers.
    */

    pop r11
    pop r10
    pop r9
    pop r8
    pop rsi
    pop rdi
    pop rdx
    pop rcx
    pop rax
    "
}

pub macro push_fs() {
    "
    /* 
    * Push FS segment.
    */
    
    push fs

    mov rcx, 0x18
    mov fs, cx
    "
}

pub macro pop_fs() {
    "
    /* 
    * Pop FS segment.
    */

    pop fs
    "
}

pub macro intel_asm($($code:expr,)+) {
    global_asm!(concat!($($code),+,));
}

pub macro const_unsafe($($vis:vis const $name:ident: $ty:ty = $value:expr;)*) {
    $(
        $vis const $name: $ty = unsafe { $value };
    )*
}

pub macro intel_fn {
    (
        $(#![$total:meta])*

        $(#[$outer:meta])* $fn_vis:vis extern "asm" fn $name:ident($($arg_name:ident : $arg_type:ty),*) { $($body:expr,)+ }
        $(pub extern "asm" $label_name:expr => { $($label_body:expr,)+ })*
    ) => {
        $(#[$total])*
        crate::utils::intel_asm!(
            ".global ", stringify!($name), "\n",
            ".type ", stringify!($name), ", @function\n",
            ".section .text.", stringify!($name), ", \"ax\", @progbits\n",
            stringify!($name), ":\n",
            $($body),+,
            $(
                stringify!($label_name), ":\n",
                $($label_body),+,
            )*
            ".size ", stringify!($name), ", . - ", stringify!($name), "\n",
            ".text\n",
        );

        $(#[$total])*
        extern "C" {
            $(#[$outer])*
            $fn_vis fn $name($($arg_name : $arg_type),*);
        }
    }
}

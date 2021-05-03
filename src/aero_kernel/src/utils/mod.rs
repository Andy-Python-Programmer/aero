pub mod io;

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

pub macro intel_asm($($code:expr,)+) {
    global_asm!(
        concat!(
            ".intel_syntax noprefix\n",
            $($code),+,
            ".att_syntax prefix\n",
        )
    );
}

pub macro intel_fn {
    (pub __asm__ volatile fn $name:ident() { $($body:expr,)+ }) => {
        crate::utils::intel_asm!(
            ".global ", stringify!($name), "\n",
            ".type ", stringify!($name), ", @function\n",
            ".section .text.", stringify!($name), ", \"ax\", @progbits\n",
            stringify!($name), ":\n",
            $($body),+,
            ".size ", stringify!($name), ", . - ", stringify!($name), "\n",
            ".text\n",
        );

        extern "C" {
            pub fn $name();
        }
    },

    (pub __asm__ volatile fn $name:ident() { $($body:expr,)+ } $(__label__ volatile $label_name:expr => { $($label_body:expr,)+ })+) => {
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

        extern "C" {
            pub fn $name();
        }
    }
}

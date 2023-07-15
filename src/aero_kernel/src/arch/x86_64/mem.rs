// Copyright (C) 2021-2023 The Aero Project Developers.
//
// This file is part of The Aero Project.
//
// Aero is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Aero is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Aero. If not, see <https://www.gnu.org/licenses/>.

fn should_store_by_byte() -> bool {
    let cpuid = raw_cpuid::CpuId::new();
    if let Some(features) = cpuid.get_extended_feature_info() {
        // Check if "Enhanced" or "Fast Short" optimizations are available.
        features.has_rep_movsb_stosb()
    } else {
        false
    }
}

#[naked]
unsafe extern "C" fn memcpy_movsq(dest: *mut u8, src: *const u8, len: usize) -> *mut u8 {
    // Registers used:
    //
    // %rdi = argument 1, `dest`
    // %rsi = argument 2, `src`
    // %rdx = argument 3, `len`
    asm!(
        // Save the return value.
        "mov rax, rdi",
        // Copy in 8 byte chunks.
        "mov rcx, rdx",
        "shr rcx, 3",
        "rep movsq",
        // Copy the rest.
        "mov rcx, rdx",
        "and rcx, 0x7",
        "rep movsb",
        "ret",
        options(noreturn)
    );
}

#[naked]
unsafe extern "C" fn memcpy_movsb(dest: *mut u8, src: *const u8, len: usize) -> *mut u8 {
    // Registers used:
    //
    // %rdi = argument 1, `dest`
    // %rsi = argument 2, `src`
    // %rdx = argument 3, `len`
    asm!(
        // Save the return value.
        "mov rax, rdi",
        // Copy!
        "mov rcx, rdx",
        "rep movsb",
        "ret",
        options(noreturn)
    )
}

#[indirect]
extern "C" fn memcpy() -> fn(*mut u8, *const u8, usize) {
    if should_store_by_byte() {
        memcpy_movsb
    } else {
        memcpy_movsq
    }
}

#[naked]
unsafe extern "C" fn memset_stosq(dest: *mut u8, byte: i32, len: usize) -> *mut u8 {
    // Registers used:
    //
    // %rdi = argument 1, `dest`
    // %rsi = argument 2, `byte`
    // %rdx = argument 3, `len`
    asm!(
        // Save the return value.
        "mov r11, rdi",
        // Create an 8-byte copy of the pattern.
        "mov rcx, rdx",
        "movzx rax, sil",
        "mov r10, 0x0101010101010101",
        "mul r10",
        "mov rdx, rcx",
        // Copy in 8 byte chunks.
        "shr rcx, 3",
        "rep stosq",
        // Copy the rest.
        "mov rcx, rdx",
        "and rcx, 0x7",
        "rep stosb",
        // Restore the return value.
        "mov rax, r11",
        "ret",
        options(noreturn)
    )
}

#[naked]
unsafe extern "C" fn memset_stosb(dest: *mut u8, byte: i32, len: usize) -> *mut u8 {
    // Registers used:
    //
    // %rdi = argument 1, `dest`
    // %rsi = argument 2, `byte`
    // %rdx = argument 3, `len`
    asm!(
        // Save the return value.
        "mov r11, rdi",
        "mov al, sil",
        "mov rcx, rdx",
        "rep stosb",
        "mov rax, r11",
        "ret",
        options(noreturn)
    )
}

#[indirect]
extern "C" fn memset() -> fn(*mut u8, i32, usize) {
    if should_store_by_byte() {
        memset_stosb
    } else {
        memset_stosq
    }
}

#[no_mangle]
#[naked]
unsafe extern "C" fn memmove_erms(dest: *mut u8, src: *const u8, len: usize) -> *mut u8 {
    // Registers used:
    //
    // %rdi = argument 1, `dest`
    // %rsi = argument 2, `src`
    // %rdx = argument 3, `len`
    asm!(
        "mov rax, rdi",
        // Skip zero length.
        "test rdx, rdx",
        "jz 2f",
        // Copying forwards:
        "mov rcx, rdx",
        "cmp rdi, rsi",
        "jb 1f",
        // `src` == `dest`
        "je 2f",
        "lea rdx, [rsi + rcx]",
        "cmp rdi, rdx",
        "jb 3f",
        "1:",
        "rep movsb",
        "2:",
        "ret",
        // Copying backwards:
        "3:",
        "lea rdi, [rdi + rcx - 1]",
        "lea rsi, [rsi + rcx - 1]",
        "std",
        "rep movsb",
        "cld",
        "ret",
        options(noreturn)
    )
}

#[no_mangle]
extern "C" fn memmove(dest: *mut u8, src: *const u8, len: usize) -> *mut u8 {
    unsafe { memmove_erms(dest, src, len) }
}

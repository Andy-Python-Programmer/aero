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

#[no_mangle]
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

#[no_mangle]
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

// FIXME(andypython): pick the best implementation for the current CPU using indirect functions.

#[no_mangle]
extern "C" fn memcpy(dest: *mut u8, src: *const u8, len: usize) -> *mut u8 {
    unsafe { memcpy_movsq(dest, src, len) }
}

#[no_mangle]
extern "C" fn memset(dest: *mut u8, byte: i32, len: usize) -> *mut u8 {
    unsafe { memset_stosq(dest, byte, len) }
}

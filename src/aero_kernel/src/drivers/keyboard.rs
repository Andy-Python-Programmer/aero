/*
 * Copyright 2021 The Aero Project Developers. See the COPYRIGHT
 * file at the top-level directory of this project.
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 * <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
 * option. This file may not be copied, modified, or distributed
 * except according to those terms.
 */

use crate::prelude::*;

const LEFT_SHIFT_PRESSED: u8 = 0x2A;
const LEFT_SHIFT_RELEASED: u8 = LEFT_SHIFT_PRESSED + 0x80;

const RIGHT_SHIFT_PRESSED: u8 = 0x36;
const RIGHT_SHIFT_RELEASED: u8 = RIGHT_SHIFT_PRESSED + 0x80;

const SPACEBAR_PRESSED: u8 = 0x39;
const ENTER_PRESSED: u8 = 0x1C;
const BACKSPACE_PRESSED: u8 = 0x0E;

const ASCII_TABLE: [char; 58] = [
    '\0', '\0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-', '=', '\0', '\0', 'q', 'w',
    'e', 'r', 't', 'y', 'u', 'i', 'o', 'p', '[', ']', '\0', '\0', 'a', 's', 'd', 'f', 'g', 'h',
    'j', 'k', 'l', ';', '\'', '`', '\0', '\\', 'z', 'x', 'c', 'v', 'b', 'n', 'm', ',', '.', '/',
    '\0', '*', '\0', ' ',
];

static mut IS_LEFT_SHIFT_PRESSED: bool = false;
static mut IS_RIGHT_SHIFT_PRESSED: bool = false;

pub fn translate_keystroke(scancode: u8, uppercase: bool) -> char {
    let scancode = scancode as usize;

    if scancode > ASCII_TABLE.len() {
        '\0'
    } else if uppercase {
        core::char::from_u32(ASCII_TABLE[scancode] as u32 - 32).unwrap()
    } else {
        ASCII_TABLE[scancode]
    }
}

pub unsafe fn handle(scancode: u8) {
    match scancode {
        LEFT_SHIFT_PRESSED => {
            IS_LEFT_SHIFT_PRESSED = true;
            return;
        }
        LEFT_SHIFT_RELEASED => {
            IS_LEFT_SHIFT_PRESSED = false;
            return;
        }

        RIGHT_SHIFT_PRESSED => {
            IS_RIGHT_SHIFT_PRESSED = true;
            return;
        }
        RIGHT_SHIFT_RELEASED => {
            IS_RIGHT_SHIFT_PRESSED = false;
            return;
        }

        ENTER_PRESSED => {
            println!();
            return;
        }

        SPACEBAR_PRESSED => {
            print!(" ");
            return;
        }

        BACKSPACE_PRESSED => {
            return;
        }

        _ => (),
    }

    let ascii_char = translate_keystroke(scancode, IS_LEFT_SHIFT_PRESSED | IS_RIGHT_SHIFT_PRESSED);

    if ascii_char != '\0' {
        print!("{}", ascii_char);
    }
}

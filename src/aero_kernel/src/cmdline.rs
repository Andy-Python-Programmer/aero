// Copyright (C) 2021-2024 The Aero Project Developers.
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

use core::num::ParseIntError;

use spin::Once;

use limine::file::File;

use crate::rendy;

static RAW_CMDLINE_STR: Once<&'static str> = Once::new();

pub struct CommandLine {
    /// If set, then the kernel logs will be redirected onto the framebuffer until
    /// the kernel thread jumps to userland.
    ///
    /// By default, the kernel logs are not redirected.
    pub rendy_debug: bool,
    pub term_background: Option<&'static [u8]>,
    pub theme_background: u32,
}

impl CommandLine {
    fn new() -> Self {
        Self {
            rendy_debug: false,
            term_background: None,
            theme_background: rendy::DEFAULT_THEME_BACKGROUND,
        }
    }
}

fn resolve_module(modules: &[&File], name: &str) -> &'static [u8] {
    modules
        .iter()
        .find(|m| {
            let n = core::str::from_utf8(m.cmdline()).unwrap();
            n == name
        })
        .map(|m| unsafe { core::slice::from_raw_parts(m.addr(), m.size() as usize) })
        .expect("resolve_module: invalid operand")
}

fn parse_number(mut string: &str) -> Result<usize, ParseIntError> {
    let is_hex = string.starts_with("0x");
    let is_octal = string.starts_with("0o");

    if is_hex {
        string = string.trim_start_matches("0x");
        usize::from_str_radix(string, 16)
    } else if is_octal {
        string = string.trim_start_matches("0o");
        usize::from_str_radix(string, 8)
    } else {
        string.parse::<usize>()
    }
}

pub fn parse(cmdline: &'static str, modules: &[&File]) -> CommandLine {
    RAW_CMDLINE_STR.call_once(|| cmdline);

    // Chew up the leading spaces.
    let cmdline = cmdline.trim();
    let mut result = CommandLine::new();

    let bail = |argument| log::warn!("unknown kernel command line option: '{}'", argument);

    for argument in cmdline.split_whitespace() {
        match argument {
            "rendy-dbg" => result.rendy_debug = true,

            _ => {
                let mut pair = argument.splitn(2, '=');

                match pair.next() {
                    Some(name) => {
                        let value = pair.next().expect("missing operand");

                        match name {
                            "term-background" => {
                                result.term_background = Some(resolve_module(modules, value))
                            }

                            "theme-background" => {
                                let theme_bg = parse_number(value).unwrap_or_else(|e| {
                                    log::warn!(
                                        "parse_number: invalid operand {}, defaulting to {}",
                                        e,
                                        rendy::DEFAULT_THEME_BACKGROUND
                                    );

                                    rendy::DEFAULT_THEME_BACKGROUND as usize
                                });

                                result.theme_background = theme_bg as u32;
                            }

                            _ => bail(argument),
                        }
                    }

                    None => bail(argument),
                }
            }
        }
    }

    result
}

/// Returns the raw kernel command line string.
///
/// ## Panics
/// * If this function was invoked before the kernel command line was parsed using [`self::parse`].
pub fn get_raw_cmdline() -> &'static str {
    RAW_CMDLINE_STR
        .get()
        .expect("get_raw_cmdline: called before cmdline was parsed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_parser_test() {
        assert_eq!(parse_number("0xdeadbeef").unwrap(), 0xdeadbeef);
        assert_eq!(parse_number("0o546").unwrap(), 0o546);
        assert_eq!(parse_number("123").unwrap(), 123);

        assert!(parse_number("invalid").is_err());
        assert!(parse_number("0xinvalid").is_err());
        assert!(parse_number("0oinvalid").is_err());
    }
}

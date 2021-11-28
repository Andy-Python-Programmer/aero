use core::num::ParseIntError;

use stivale_boot::v2::StivaleModuleTag;

use crate::rendy;

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

fn resolve_module(modules: &'static StivaleModuleTag, name: &str) -> &'static [u8] {
    modules
        .iter()
        .find(|m| m.as_str() == name)
        .map(|m| unsafe { core::slice::from_raw_parts(m.start as *const u8, m.size() as usize) })
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
        usize::from_str_radix(string, 10)
    }
}

pub fn parse(cmdline: &str, modules: &'static StivaleModuleTag) -> CommandLine {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[aero_test::test]
    fn number_parser_test() {
        assert_eq!(parse_number("0xdeadbeef").unwrap(), 0xdeadbeef);
        assert_eq!(parse_number("0o546").unwrap(), 0o546);
        assert_eq!(parse_number("123").unwrap(), 123);

        assert!(parse_number("invalid").is_err());
        assert!(parse_number("0xinvalid").is_err());
        assert!(parse_number("0oinvalid").is_err());
    }
}

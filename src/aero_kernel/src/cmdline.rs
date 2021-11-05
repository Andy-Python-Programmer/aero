use stivale_boot::v2::StivaleModuleTag;

pub struct CommandLine {
    /// If set, then the kernel logs will be redirected onto the framebuffer until
    /// the kernel thread jumps to userland.
    ///
    /// By default, the kernel logs are not redirected.
    pub rendy_debug: bool,
    pub term_background: Option<&'static [u8]>,
}

impl CommandLine {
    pub fn new() -> Self {
        Self {
            rendy_debug: false,
            term_background: None,
        }
    }
}

fn resolve_module(modules: &'static StivaleModuleTag, name: &str) -> &'static [u8] {
    modules
        .iter()
        .find(|m| m.as_str() == name)
        .map(|m| unsafe { core::slice::from_raw_parts(m.start as *const u8, m.size() as usize) })
        .expect("invalid operand")
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
                            "--term-background" => {
                                result.term_background = Some(resolve_module(modules, value))
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

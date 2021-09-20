pub struct CommandLine {
    /// If set, then the kernel logs will be redirected onto the framebuffer until
    /// the kernel thread jumps to userland.
    ///
    /// By default, the kernel logs are not redirected.
    pub rendy_debug: bool,
}

impl CommandLine {
    pub fn new() -> Self {
        Self { rendy_debug: false }
    }
}

pub fn parse(cmdline: &str) -> CommandLine {
    // Chew up the leading spaces.
    let cmdline = cmdline.trim();
    let mut result = CommandLine::new();

    for argument in cmdline.split_whitespace() {
        match argument {
            "rendy-dbg" => result.rendy_debug = true,
            _ => log::warn!("unknown kernel command line option: '{}'", argument),
        }
    }

    result
}

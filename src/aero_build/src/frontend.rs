use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(name = "test", about, version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(long, global = true, help = "Remove all build artifacts")]
    pub clean: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Build(BuildArgs),
    /// Generate documentation for the aero kernel
    Docs,
    /// Run the aero test suite
    Test,
    Testing,
}

#[derive(Args, Debug)]
pub struct BuildArgs {
    #[arg(
        short,
        long,
        help = "Check if aero builds correctly without packaging and running it"
    )]
    pub check: bool,

    #[arg(short, long, help = "Build the kernel and userland in debug mode")]
    pub debug: bool,

    #[arg(long, value_enum, default_value_t = BuildMode::BuildAndRun, help = "Either only builds aero, only runs aero in an emulator, or both")]
    pub mode: BuildMode,

    #[arg(short, long, value_enum, default_value_t = BIOS::Legacy, help = "Override firmware that the emulator will use")]
    pub bios: BIOS,

    #[arg(
        short,
        long,
        use_value_delimiter = true,
        value_delimiter = ',',
        help = "Additional features to build the kernel with, seperated by commas"
    )]
    pub features: Vec<String>,

    #[arg(short, long, default_value_t = String::from("x86_64-aero_os"), help = "Override the target triple that the kernel will be built for")]
    pub target: String,
    
    #[arg(
        short,
        long,
        help = "Run the emulator with 5 level paging support if applicable"
    )]
    pub la57: bool,

    #[arg(
        short,
        long,
        help = "Build the full userland sysroot. If disabled, the sysroot will only contain the aero_shell and the init binaries"
    )]
    pub sysroot: bool,

    #[arg(
        short = 'k',
        long,
        help = "Disable KVM acceleration even if it's available"
    )]
    pub disable_kvm: bool,

    // todo: validate memory value
    #[arg(short, long, default_value_t = String::from("9800M"), help = "Amount of memory to allocate to QEMU")]
    pub memory: String,

    #[arg(last = true, help = "Additional arguments to pass to the emulator")]
    pub emulator_args: Vec<String>,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum BuildMode {
    /// Only build the image
    OnlyBuild,
    /// Only run the build image if possible
    OnlyRun,
    /// Builds and run the image
    BuildAndRun,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum BIOS {
    Legacy,
    UEFI,
}

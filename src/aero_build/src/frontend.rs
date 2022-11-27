use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(about, version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(long, global = true, help = "Removes all build artifacts")]
    pub clean: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Build(BuildArgs),
    /// Generates documentation for the aero kernel
    Docs,
    /// Runs the aero test suite
    Test,
    Testing,
}

#[derive(Args, Debug)]
pub struct BuildArgs {
    #[arg(long, help = "Checks if aero builds correctly without packaging and running it")]
    pub check: bool,
    #[arg(long, help = "Builds the kernel and userland in debug mode")]
    pub debug: bool,
    #[arg(long, value_enum, default_value_t = RunMode::BuildAndRun, help = "Either only builds aero, only runs aero in an emulator, or both")]
    pub run_mode: RunMode,
    #[arg(long, value_enum, default_value_t = BIOS::Legacy, help = "")]
    pub bios: BIOS,
    #[arg(long, use_value_delimiter = true, value_delimiter = ',', help = "Additional features to build the kernel with, seperated by commas")]
    pub features: Vec<String>,
    #[arg(long, default_value_t = String::from("x86_64-aero_os"), help = "Override the target triple that the kernel will be built for")]
    pub target: String,
    #[arg(long, help = "Run the emulator with 5 level paging support if applicable")]
    pub la57: bool,
    #[arg(long, help = "Build the full userland sysroot. If disabled, then the sysroot will only contain the aero_shell and the init binaries")]
    pub sysroot: bool,
    #[arg(long, help = "Disable KVM acceleration even if it's available")]
    pub disable_kvm: bool,
    // todo: validate memory value
    #[arg(long, default_value_t = String::from("9800M"), help = "Amount of memory to allocate to QEMU")]
    pub memory: String,
    // todo: clarify that these additional args are passed as: --emulator-args="arg1 arg2"
    #[arg(long, help = "Additional arguments to pass to the emulator")]
    pub emulator_args: Option<String>,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum RunMode {
    /// Only builds the image
    OnlyBuild,
    /// Only runs the build image if possible
    OnlyRun,
    /// Builds and runs the image
    BuildAndRun,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum BIOS {
    Legacy,
    UEFI,
}
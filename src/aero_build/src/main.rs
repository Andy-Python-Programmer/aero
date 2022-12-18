#![cfg(not(windows))]

use clap::Parser;
use stopwatch::Stopwatch;

use crate::frontend::{Cli, Commands, BIOS};

mod build;
mod constants;
mod docs;
mod frontend;
mod test;
mod utils;

fn main() {
    let sw = Stopwatch::start_new();
    let cli = Cli::parse();

    match &cli.command {
        Commands::Build(args) => {
            build::build(&cli, &args);
        }
        Commands::Docs => {}
        Commands::Test => {
            test::test(&cli);
        }
        Commands::Testing => {
            testing();
        }
    }
}

fn testing() {
    // use std::process::{Command, Stdio};
    // use execute::{Execute, shell};

    // let mut command = shell("./aero.py");
    // command.stdout(Stdio::piped());
    // let output = command.execute_output().unwrap();
    // println!("{}", String::from_utf8(output.stdout).unwrap());
}

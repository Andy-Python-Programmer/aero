use std::env;

use clap::Parser;

use crate::frontend::{Cli, Commands};

mod build;
mod constants;
mod frontend;
mod test;
mod utils;

fn main() {
    let cli = Cli::parse();

    if cli.clean {
        // clean
    }

    match &cli.command {
        Commands::Build(args) => {
            println!("{:#?}", args);
            // build::build(&cli, args);

        },
        Commands::Docs => {},
        Commands::Test => {
            test::test(&cli);
        },
        Commands::Testing => {
            testing();
        },
    }
}

fn testing() {
    use std::process::{Command, Stdio};

    use execute::{Execute, shell};
    
    let mut command = shell("./aero.py");
    
    command.stdout(Stdio::piped());
    
    let output = command.execute_output().unwrap();
    
    println!("{}", String::from_utf8(output.stdout).unwrap());
}

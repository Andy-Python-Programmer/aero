use std::path::Path;
use std::process::Command;

use crate::utils;
use crate::constants;

struct BuildInfo {
    args: Vec<String>,
    target_arch: String,
}

pub fn build() {
    utils::log_info("build test");

    let output = utils::run_command(&std::env::current_dir().unwrap(), "/usr/bin/ls", vec![String::from("-l"), String::from("-a")]);
    println!("{:?}", output);
    output.log_if_exists();
}

fn build_cargo_workspace(cwd: &Path, command: &str, args: Vec<String>, cargo: Option<&str>) {
    // let cargo_cmd = cargo.unwrap_or("cargo");

    // Command::new(cargo_cmd)
    //     .arg(command)
    //     .args(args)
    //     .current_dir(cwd)
    //     .spawn()
    //     .expect("cargo failed to run");

    // let code, _, _ = run_command([cargo.unwrap_or("cargo"), command, *args], cwd=cwd);

    // if code != 0:
    //     return None

    // _, stdout, _ = run_command([cargo, command, *args, '--message-format=json'],
    //                            stdout=subprocess.PIPE,
    //                            stderr=subprocess.DEVNULL,
    //                            cwd=cwd)

    // return extract_artifacts(stdout);
}
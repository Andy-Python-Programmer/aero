use std::process::{Command, ExitStatus, Stdio};
use std::path::Path;
use std::io::{BufReader, BufRead};

use execute::Execute;

/// Logs a message with info log level.
pub fn log_info(message: &str) {
    println!("\x1b[1;32minfo\x1b[0m: {message}");
}

/// Logs a message with error log level.
pub fn log_error(message: &str) {
    println!("\x1b[1;31merror\x1b[0m: {message}");
}

#[derive(Debug)]
pub struct CommandOutput {
    pub exit_status: ExitStatus,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
}

impl CommandOutput {
    pub fn log_if_exists(&self) {
        if let Some(stdout) = &self.stdout {
            log_info(&stdout);
        }
        if let Some(stderr) = &self.stderr {
            log_error(&stderr);
        }
    }
}

pub fn run_command(command: &str, args: Vec<&str>, pwd: Option<&Path>) -> CommandOutput {
    let current_dir = std::env::current_dir().unwrap();
    let pwd = pwd.unwrap_or(current_dir.as_path());

    let output = Command::new(command)
        .args(args)
        .current_dir(pwd)
        .output()
        .expect("failed to execute process");

    let stdout = if !&output.stdout.is_empty() {
        Some(String::from_utf8(output.stdout).unwrap())
    } else {
        None
    };
    let stderr = if !&output.stderr.is_empty() {
        Some(String::from_utf8(output.stderr).unwrap())
    } else {
        None
    };

    CommandOutput {
        exit_status: output.status,
        stdout: stdout,
        stderr: stderr,
    }
}

use std::process::{Command, ExitStatus};
use std::path::Path;

const ANSI_ESCAPE: &str = "\x1b[";
const ANSI_BOLD_RED: &str = "1;31m";
const ANSI_BOLD_GREEN: &str = "1;32m";
const ANSI_RESET: &str = "0m";

pub fn log_info(message: &str) {
    println!("{ANSI_ESCAPE}{ANSI_BOLD_GREEN}info: {ANSI_ESCAPE}{ANSI_RESET}{message}");
}

pub fn log_error(message: &str) {
    println!("{ANSI_ESCAPE}{ANSI_BOLD_RED}red: {ANSI_ESCAPE}{ANSI_RESET}{message}");
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

pub fn run_command(pwd: &Path, command: &str, args: Vec<String>) -> CommandOutput {
    let output = Command::new(command)
        .arg(command)
        .args(args)
        // .current_dir(pwd)
        .output()
        .expect("todo");

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

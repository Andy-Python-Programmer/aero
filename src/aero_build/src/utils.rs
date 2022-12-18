use std::io::{self, BufRead, BufReader};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

use crate::constants;

// use execute::Execute;

/// Logs a message with info log level.
pub fn log_info<S: Into<String>>(message: S) {
    println!("\x1b[1;32minfo\x1b[0m: {}", message.into());
}

/// Logs a message with error log level.
pub fn log_error<S: Into<String>>(message: S) {
    println!("\x1b[1;31merror\x1b[0m: {}", message.into());
}

/// Logs a message with debug log level.
pub fn log_debug<S: Into<String>>(message: S) {
    if std::env::var("AERO_BUILD_DEBUG").is_ok() {
        println!("\x1b[1;35mdebug\x1b[0m: {}", message.into());
    }
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
            log_info(stdout);
        }
        if let Some(stderr) = &self.stderr {
            log_error(stderr);
        }
    }
}

pub fn run_command<C, I, S>(
    command: C,
    args: I,
    pwd: Option<&Path>,
) -> Result<CommandOutput, io::Error>
where
    C: Into<String>,
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let current_dir = std::env::current_dir().unwrap();
    let pwd = pwd.unwrap_or(current_dir.as_path());

    let cmd = Command::new(command.into()).args(args).current_dir(pwd).output();

    let output = match cmd {
        Ok(output) => output,
        Err(err) => {
            return Err(err);
        }
    };

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

    Ok(CommandOutput {
        exit_status: output.status,
        stdout: stdout,
        stderr: stderr,
    })
}

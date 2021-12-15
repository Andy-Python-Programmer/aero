use aero_syscall::*;
use core::fmt::{Result as FmtResult, Write};

pub struct Stdout;
pub struct Stderr;

impl Write for Stdout {
    fn write_str(&mut self, str: &str) -> FmtResult {
        sys_write(1, str.as_bytes()).unwrap();

        Ok(())
    }
}

impl Write for Stderr {
    fn write_str(&mut self, str: &str) -> FmtResult {
        sys_write(2, str.as_bytes()).unwrap();

        Ok(())
    }
}

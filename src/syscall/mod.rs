pub mod fs;

pub use fs::*;

pub enum SyscallError {
    /// Operation not permitted.
    NotPermitted,
    /// No such file or directory.
    NoEntry,
    /// Invalid argument.
    InvalidValue,
    /// Syscall not implemented.
    NoCall,
}

pub type SyscallResult<T> = Result<T, SyscallError>;

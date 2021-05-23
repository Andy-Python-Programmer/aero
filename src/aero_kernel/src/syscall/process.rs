use aero_syscall::AeroSyscallError;

pub fn exit(status: usize) -> Result<usize, AeroSyscallError> {
    log::debug!("Exiting the current process with status: {}", status);

    Err(AeroSyscallError::Unknown)
}

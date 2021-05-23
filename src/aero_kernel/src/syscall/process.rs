use aero_syscall::AeroSyscallError;

use crate::userland::scheduler;

pub fn exit(status: usize) -> Result<usize, AeroSyscallError> {
    log::debug!("Exiting the current process with status: {}", status);

    let scheduler = scheduler::get_scheduler();

    Err(AeroSyscallError::Unknown)
}

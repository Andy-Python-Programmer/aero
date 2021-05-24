use aero_syscall::AeroSyscallError;

use crate::userland::scheduler;

pub fn write(fd: usize, buf: usize, len: usize) -> Result<usize, AeroSyscallError> {
    log::debug!("sys_write(fd={:#x}, buf={:#x}, len={:#x})", fd, buf, len);

    let current_task = scheduler::get_scheduler()
        .active_task_ref()
        .expect("`sys_write` was invoked with no active tasks running");

    current_task
        .file_table
        .get_handle(fd)
        .ok_or(AeroSyscallError::Unknown)?;

    Ok(0)
}

use aero_syscall::AeroSyscallError;

use crate::{fs::Path, userland::scheduler, utils::validate_str};

pub fn write(fd: usize, buf: usize, len: usize) -> Result<usize, AeroSyscallError> {
    log::trace!("sys_write(fd={:#x}, buf={:#x}, len={:#x})", fd, buf, len);

    let current_task = scheduler::get_scheduler()
        .active_task_ref()
        .expect("`sys_write` was invoked with no active tasks running");

    current_task
        .file_table
        .get_handle(fd)
        .ok_or(AeroSyscallError::EBADFD)?;

    Ok(0)
}

pub fn open(path: usize, len: usize, mode: usize) -> Result<usize, AeroSyscallError> {
    log::trace!("sys_open(path={:#x}, len={:#x}, mode={:#x})", path, len, mode);

    if let Some(path) = validate_str(path as *mut _, len) {
        let _ = match Path::new(path) {
            Ok(path) => path,
            Err(_) => return Err(AeroSyscallError::EINVAL),
        };

        Ok(0)
    } else {
        Err(AeroSyscallError::EINVAL)
    }
}

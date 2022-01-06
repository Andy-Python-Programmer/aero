use aero_syscall::*;

use crate::{fs::inode::DirEntry, socket::unix::*, userland::scheduler};

pub fn socket(
    domain: usize,
    socket_type: usize,
    protocol: usize,
) -> Result<usize, AeroSyscallError> {
    let socket = match (domain, socket_type, protocol) {
        (AF_UNIX, SOCK_STREAM, 0) => UnixSocket::new(),
        (_, _, _) => {
            log::warn!(
                "unsupported socket type: domain={}, socket_type={}, protocol={}",
                domain,
                socket_type,
                protocol
            );

            return Err(AeroSyscallError::EINVAL);
        }
    };

    let entry = DirEntry::from_inode(socket);
    let current_task = scheduler::get_scheduler().current_task();
    let fd = current_task
        .file_table
        .open_file(entry, OpenFlags::empty())?;

    Ok(fd)
}

pub fn bind(fd: usize, address: usize, length: usize) -> Result<usize, AeroSyscallError> {
    let address = unsafe { &*(address as *const SocketAddr) };
    let current_task = scheduler::get_scheduler().current_task();
    let file = current_task.file_table.get_handle(fd);

    match file {
        Some(handle) => {
            if handle.inode().metadata()?.is_socket() {
                handle.inode().bind(address, length)?;

                Ok(0)
            } else {
                Err(AeroSyscallError::ENOTSOCK)
            }
        }
        None => Err(AeroSyscallError::ENOENT),
    }
}

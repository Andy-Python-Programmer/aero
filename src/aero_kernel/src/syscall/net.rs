use aero_syscall::*;

use crate::fs::inode::DirEntry;
use crate::mem::paging::VirtAddr;

use crate::socket::unix::*;
use crate::socket::SocketAddr;

use crate::userland::scheduler;

/// Creates a [`SocketAddr`] from the provided userland socket structure address. This
/// is done by looking at the family field present in every socket address structure.
fn socket_addr_from_addr<'sys>(address: VirtAddr) -> Result<SocketAddr<'sys>, AeroSyscallError> {
    let family = address
        .read_mut::<u32>()
        .ok_or(AeroSyscallError::EINVAL)?
        .clone();

    Ok(SocketAddr::from_family(address, family).ok_or(AeroSyscallError::EINVAL)?)
}

/// Connects the socket to the specified address.
#[syscall]
pub fn connect(fd: usize, address: usize, length: usize) -> Result<usize, AeroSyscallError> {
    let address = socket_addr_from_addr(VirtAddr::new(address as u64))?;
    let file = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(AeroSyscallError::EINVAL)?;

    file.inode().connect(address, length)?;
    Ok(0)
}

/// Marks the socket as a passive socket (i.e. as a socket that will be used to accept incoming
/// connection requests).
#[syscall]
pub fn listen(fd: usize, backlog: usize) -> Result<usize, AeroSyscallError> {
    let file = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(AeroSyscallError::EINVAL)?;

    file.inode().listen(backlog)?;
    Ok(0)
}

#[syscall]
pub fn socket(
    domain: usize,
    socket_type: usize,
    protocol: usize,
) -> Result<usize, AeroSyscallError> {
    let socket = match (domain as u32, socket_type, protocol) {
        (AF_UNIX, SOCK_STREAM, 0) => UnixSocket::new(),
        (_, _, _) => {
            log::warn!(
                "unsupported socket type: domain={domain}, socket_type={socket_type}, protocol={protocol}"
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

#[syscall]
pub fn bind(fd: usize, address: usize, length: usize) -> Result<usize, AeroSyscallError> {
    let address = socket_addr_from_addr(VirtAddr::new(address as u64))?;

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

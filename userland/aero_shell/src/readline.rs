use aero_syscall::*;

pub fn readline() -> Result<String, AeroSyscallError> {
    let tty_fd = sys_open("/dev/tty", OpenFlags::O_RDONLY)?;

    let mut orig_termios = Termios::default();
    let mut buffer = String::new();

    sys_ioctl(tty_fd, TCGETS, &mut orig_termios as *mut _ as usize)?;

    let mut termios = orig_termios.clone();
    termios
        .c_lflag
        .remove(TermiosLFlag::ECHO | TermiosLFlag::ICANON);

    sys_ioctl(tty_fd, TCSETSF, &termios as *const _ as usize)?;

    // Now we are in raw TTY mode, say vola :^)
    loop {
        let mut buf = [0; 1];
        let size = sys_read(tty_fd, &mut buf)?;

        if size == 0 {
            break;
        }

        if buf[0] == b'\n' {
            break;
        }

        let char = buf[0] as char;

        print!("{:?}", char);
        buffer.push(char);
    }

    sys_ioctl(tty_fd, TCSETSF, &orig_termios as *const _ as usize)?;
    sys_close(tty_fd)?;

    Ok(buffer)
}

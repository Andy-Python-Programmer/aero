use aero_syscall::*;

enum KeyEvent {
    Raw(char),

    Up,
    Down,
    Left,
    Right,
}

struct RawReader {
    tty_fd: usize,
}

impl RawReader {
    fn read(&self) -> Option<KeyEvent> {
        let c = self.next_char()?;

        if c == '\x1b' {
            let c = self.next_char()?;

            if c == '[' {
                let c = self.next_char()?;

                match c {
                    'A' => Some(KeyEvent::Up),
                    'B' => Some(KeyEvent::Down),
                    'C' => Some(KeyEvent::Right),
                    'D' => Some(KeyEvent::Left),
                    _ => None,
                }
            } else {
                Some(KeyEvent::Raw(c))
            }
        } else {
            Some(KeyEvent::Raw(c))
        }
    }

    fn next_char(&self) -> Option<char> {
        let mut buf = [0; 1];
        let size = sys_read(self.tty_fd, &mut buf).ok()?;

        if size == 0 {
            None
        } else if buf[0] == b'\n' {
            None
        } else {
            Some(buf[0] as char)
        }
    }
}

pub fn readline(prefix: &str, history: &Vec<String>) -> Result<String, AeroSyscallError> {
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
    let reader = RawReader { tty_fd };

    let mut resolution = WinSize::default();
    sys_ioctl(tty_fd, TIOCGWINSZ, &mut resolution as *mut _ as usize)?;

    let mut history_i = history.len();

    print!("{}", prefix);

    loop {
        if let Some(key) = reader.read() {
            if let KeyEvent::Raw(eee) = key {
                buffer.push(eee);
                print!("{}", eee);
            } else if let KeyEvent::Up = key {
                if history_i >= 1 {
                    buffer.clear();

                    // Move to the start of the line
                    print!("\r");

                    // Clear the line
                    for _ in 0..resolution.ws_col - 1 {
                        print!(" ");
                    }

                    // Now that we have cleared the line move back to the start. `\r`
                    history_i -= 1;
                    buffer.push_str(history[history_i].as_str());

                    print!("\r{}{}", prefix, history[history_i]);
                }
            }
        } else {
            break;
        }
    }

    println!();

    sys_ioctl(tty_fd, TCSETSF, &orig_termios as *const _ as usize)?;
    sys_close(tty_fd)?;

    Ok(buffer)
}

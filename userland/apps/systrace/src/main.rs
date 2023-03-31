use std::env;
use std::process::Command;

use std::os::unix::process::CommandExt;

use aero_syscall::syscall0;

fn main() {
    // [1..] to ignore the name of our binary.
    let args = &env::args().collect::<Vec<_>>()[1..];

    syscall0(aero_syscall::prelude::SYS_TRACE);

    Command::new(&args[0]).args(&args[1..]).exec();
    unreachable!("systrace: failed to execute target process {args:?}")
}

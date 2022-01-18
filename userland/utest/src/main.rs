/*
 * Copyright (C) 2021-2022 The Aero Project Developers.
 *
 * This file is part of The Aero Project.
 *
 * Aero is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Aero is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Aero. If not, see <https://www.gnu.org/licenses/>.
 */

#![feature(naked_functions)]

use core::sync::atomic::{AtomicUsize, Ordering};

use aero_syscall::signal::*;
use aero_syscall::*;

struct Test<'a> {
    path: &'a str,
    func: fn() -> Result<(), AeroSyscallError>,
}

static TEST_FUNCTIONS: &[&'static Test<'static>] = &[
    &clone_process,
    &forked_pipe,
    &signal_handler,
    &dup_fds,
    &dup2_redirect_stdout,
];

fn main() {
    sys_open("/dev/tty", OpenFlags::O_RDONLY).expect("Failed to open stdin");
    sys_open("/dev/tty", OpenFlags::O_WRONLY).expect("Failed to open stdout");
    sys_open("/dev/tty", OpenFlags::O_WRONLY).expect("Failed to open stderr");

    println!("running {} tests", TEST_FUNCTIONS.len());

    for test_function in TEST_FUNCTIONS {
        (test_function.func)().unwrap();
        println!("test {} ... \x1b[1;32mok\x1b[0m", test_function.path);
    }
}

#[utest_proc::test]
fn dup2_redirect_stdout() -> Result<(), AeroSyscallError> {
    let utest_fd = sys_open("utest.txt", OpenFlags::O_WRONLY | OpenFlags::O_CREAT)?;

    // We set the new_fd to the file descriptor of stdout (i.e. 1)
    sys_dup2(utest_fd, 1, OpenFlags::O_WRONLY)?;

    // Now if we write to stdout, it will be written to utest.txt
    println!("yes");

    sys_seek(utest_fd, 0, SeekWhence::SeekSet)?;

    let mut content = [0; 3];
    sys_read(utest_fd, &mut content)?;

    core::assert_eq!(&content, b"yes");

    sys_unlink(AT_FDCWD as usize, "utest.txt", OpenFlags::empty())?;
    sys_close(utest_fd)?;

    // Restore the actual stdout.
    let tty_fd = sys_open("/dev/tty", OpenFlags::O_WRONLY)?;
    sys_dup2(tty_fd, 1, OpenFlags::O_WRONLY)?;

    Ok(())
}

#[utest_proc::test]
fn dup_fds() -> Result<(), AeroSyscallError> {
    let utest_fd = sys_open("utest.txt", OpenFlags::O_WRONLY | OpenFlags::O_CREAT)?;

    // dup() will create a copy of the utest fd as cutest_fd then both can
    // be used interchangeably.
    let cutest_fd = sys_dup(utest_fd, OpenFlags::O_RDWR)?;

    sys_write(utest_fd, b"testing ")?;
    sys_write(cutest_fd, b"dup...\n")?;

    sys_seek(utest_fd, 0, SeekWhence::SeekSet)?;

    let mut content = [0; 15];
    sys_read(utest_fd, &mut content)?;

    core::assert_eq!(&content, b"testing dup...\n");

    sys_unlink(AT_FDCWD as usize, "utest.txt", OpenFlags::empty())?;

    // Close all of the fds.
    sys_close(utest_fd)?;
    sys_close(cutest_fd)?;

    Ok(())
}

#[utest_proc::test]
fn signal_handler() -> Result<(), AeroSyscallError> {
    let mut pipe = [0usize; 2];
    sys_pipe(&mut pipe, OpenFlags::empty())?;

    static PIPE_WRITE: AtomicUsize = AtomicUsize::new(0);
    PIPE_WRITE.store(pipe[1], Ordering::SeqCst);

    fn handle_segmentation_fault(fault: usize) {
        core::assert_eq!(fault, 11);

        let pfd = PIPE_WRITE.load(Ordering::SeqCst);

        // Dont worry about closing the file descriptors since they will
        // be auto closed by the parent.
        sys_write(pfd, b"yes").expect("failed to write to the pipe");
        sys_exit(0);
    }

    // Install the signal handler.
    let handler = SignalHandler::Handle(handle_segmentation_fault);
    let sigaction = SigAction::new(handler, 0, SignalFlags::empty());

    sys_sigaction(SIGSEGV, Some(&sigaction), None)
        .expect("failed to install the segmentation fault handler");

    // On fork the signal handler will be copied over to the child process.
    let child = sys_fork()?;

    if child == 0 {
        // Create a traditional page fault :^)
        unsafe {
            *(0xcafebabe as *mut usize) = 69;
        }
    } else {
        let mut buffer = [0; 3];
        sys_read(pipe[0], &mut buffer)?;

        core::assert_eq!(&buffer, b"yes");
    }

    // Close the pipe/
    sys_close(pipe[0])?;
    sys_close(pipe[1])?;

    Ok(())
}

#[utest_proc::test]
fn forked_pipe() -> Result<(), AeroSyscallError> {
    let mut pipe = [0usize; 2];
    sys_pipe(&mut pipe, OpenFlags::empty())?;

    let child = sys_fork()?;

    if child == 0 {
        sys_close(pipe[0])?; // close the read end

        sys_write(pipe[1], b"Hello, World!")?;

        sys_close(pipe[1])?; // close the write end
        sys_exit(0)
    } else {
        let mut status = 0;
        sys_waitpid(child, &mut status, 0)?;

        sys_close(pipe[1])?; // close the write end

        let mut buffer = [0; 13];
        sys_read(pipe[0], &mut buffer)?;

        core::assert_eq!(&buffer, b"Hello, World!");

        sys_close(pipe[0])?; // close the read end
    }

    Ok(())
}

// Emulates how mlibc under the hood does clone()
#[utest_proc::test]
fn clone_process() -> Result<(), AeroSyscallError> {
    const STACK_SIZE: usize = 4096;

    #[naked]
    unsafe extern "C" fn cloned_process_start() {
        core::arch::asm!(
            "
            pop rdi
            pop rsi
            pop rdx
            call cloned_process_trampoline
            ",
            options(noreturn)
        );
    }

    #[no_mangle]
    extern "C" fn cloned_process_trampoline(func: usize, arg: usize, tcb: usize) {
        core::assert_eq!(tcb, 0xcafebabe);
        core::assert_eq!(arg, 0xbabecafe);

        let ptr = func as *const ();
        let code: extern "C" fn() = unsafe { core::mem::transmute(ptr) };

        (code)();
        sys_exit(0);
    }

    extern "C" fn cloned_process() {
        println!("Hello, World from cloned process!");
    }

    // Allocate the stack for the child process.
    let stack = sys_mmap(
        0,
        STACK_SIZE,
        MMapProt::PROT_READ | MMapProt::PROT_WRITE,
        MMapFlags::MAP_PRIVATE | MMapFlags::MAP_ANONYOMUS,
        -1isize as usize,
        0,
    )?;

    let stack_top = stack + STACK_SIZE;
    let mut stack_ptr = stack_top as *mut usize;

    // Prepare the stack for the child process.
    unsafe {
        *stack_ptr = 0xcafebabe; // TCB pointer

        stack_ptr = stack_ptr.sub(1);
        *stack_ptr = 0xbabecafe; // User argument

        stack_ptr = stack_ptr.sub(1);
        *stack_ptr = cloned_process as usize; // Inner function
    }

    // Create the child process.
    let child = sys_clone(cloned_process_start as usize, stack_ptr as usize)?;

    let mut status = 0;
    sys_waitpid(child, &mut status, 0)?;

    // Free the allocated stack.
    sys_munmap(stack, STACK_SIZE)?;

    let exit_code = status & 0xff;

    if exit_code != 0 {
        core::panic!("child exited with a non-zero status code: {}", exit_code);
    }

    Ok(())
}

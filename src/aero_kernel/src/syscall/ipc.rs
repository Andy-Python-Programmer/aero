// Copyright (C) 2021-2024 The Aero Project Developers.
//
// This file is part of The Aero Project.
//
// Aero is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Aero is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Aero. If not, see <https://www.gnu.org/licenses/>.

use crate::userland::scheduler::get_scheduler;
use crate::userland::task::TaskId;

use crate::utils::sync::{Mutex, WaitQueue, WaitQueueFlags};

use aero_syscall::SyscallError;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use spin::Once;

// TODO: Make this reassignable in case we want to handle the root node's death, so
// someone else can take over (e.g. system server but after it's restarted)
static IPC_ROOT_NODE: Once<usize> = Once::new();

struct Message {
    from: usize,
    data: Vec<u8>,
}

pub struct MessageQueue {
    queue: Mutex<VecDeque<Message>>,
    blockqueue: WaitQueue,
}

impl MessageQueue {
    pub fn new() -> MessageQueue {
        MessageQueue {
            queue: Mutex::new(VecDeque::new()),
            blockqueue: WaitQueue::new(),
        }
    }
}

fn handle_receive(
    pid_ptr: &mut usize,
    output: &mut [u8],
    msg: Message,
) -> Result<usize, SyscallError> {
    output[0..msg.data.len()].copy_from_slice(&msg.data);

    *pid_ptr = msg.from;

    Ok(msg.data.len())
}

#[syscall]
pub fn send(pid: usize, payload: &[u8]) -> Result<usize, SyscallError> {
    let target = get_scheduler()
        .find_task(TaskId::new(pid))
        .ok_or(SyscallError::EINVAL)?;

    let message_queue = &target.message_queue;
    let mut queue = message_queue.queue.lock();

    // Push the message to the message queue of the provided task.
    queue.push_back(Message {
        from: get_scheduler().current_task().pid().as_usize(),
        data: payload.to_vec(),
    });

    // Notify the task that it has a new message if its awaiting for one!
    message_queue.blockqueue.notify_all();

    Ok(0)
}

#[syscall]
pub fn recv(pid_ptr: &mut usize, output: &mut [u8], block: usize) -> Result<usize, SyscallError> {
    let current = get_scheduler().current_task();

    if block == 0 {
        // nonblocking read
        let mut msgqueue = current.message_queue.queue.lock();
        let item = msgqueue
            .pop_front()
            .expect("empty message queues should always be deleted!");

        if item.data.len() > output.len() {
            msgqueue.push_front(item);
            return Err(SyscallError::E2BIG);
        }

        return handle_receive(pid_ptr, output, item);
    }

    let mq = &current.message_queue;
    let mut our_queue = mq
        .blockqueue
        .wait(WaitQueueFlags::empty(), &mq.queue, |msg| {
            msg.front().is_some()
        })
        .unwrap();

    let msg = our_queue
        .pop_front()
        .expect("ipc_receive: someone else stole our message!");

    if msg.data.len() > output.len() {
        our_queue.push_front(msg);
        Err(SyscallError::E2BIG)
    } else {
        handle_receive(pid_ptr, output, msg)
    }
}

#[syscall]
pub fn discover_root() -> Result<usize, SyscallError> {
    match IPC_ROOT_NODE.get() {
        Some(pid) => Ok(*pid),
        None => Err(SyscallError::EINVAL),
    }
}

#[syscall]
pub fn become_root() -> Result<usize, SyscallError> {
    if IPC_ROOT_NODE.is_completed() {
        Err(SyscallError::EINVAL)
    } else {
        IPC_ROOT_NODE.call_once(|| get_scheduler().current_task().pid().as_usize());

        Ok(0)
    }
}

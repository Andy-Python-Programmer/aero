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

use crate::userland::scheduler::get_scheduler;
use crate::userland::task::TaskId;

use crate::utils::sync::{BlockQueue, Mutex};
use crate::utils::validate_slice_mut;

use aero_syscall::AeroSyscallError;
use alloc::{collections::VecDeque, vec::Vec};

struct Message {
    from: usize,
    data: Vec<u8>,
}

pub struct MessageQueue {
    queue: Mutex<VecDeque<Message>>,
    blockqueue: BlockQueue,
}

impl MessageQueue {
    pub fn new() -> MessageQueue {
        MessageQueue {
            queue: Mutex::new(VecDeque::new()),
            blockqueue: BlockQueue::new(),
        }
    }
}

fn handle_recieve(
    pid: usize,
    message_ptr: usize,
    message_size: usize,
    msg: Message,
) -> Result<usize, AeroSyscallError> {
    let pid_ptr = pid as *mut usize;
    let output =
        validate_slice_mut(message_ptr as *mut u8, message_size).ok_or(AeroSyscallError::EINVAL)?;

    output[0..msg.data.len()].copy_from_slice(&msg.data);

    unsafe {
        pid_ptr.write(msg.from);
    }

    Ok(msg.data.len())
}

pub fn send(pid: usize, message: usize, message_size: usize) -> Result<usize, AeroSyscallError> {
    // Verify and convert the provided message into a slice.
    let payload =
        validate_slice_mut(message as *mut u8, message_size).ok_or(AeroSyscallError::EINVAL)?;

    let target = get_scheduler()
        .find_task(TaskId::new(pid))
        .ok_or(AeroSyscallError::EINVAL)?;

    let message_queue = &target.message_queue;
    let mut queue = message_queue.queue.lock();

    // Push the message to the message queue of the provided task.
    queue.push_back(Message {
        from: get_scheduler().current_task().pid().as_usize(),
        data: payload.to_vec(),
    });

    // Notify the task that it has a new message if its awaiting for one!
    message_queue.blockqueue.notify_complete();

    Ok(0)
}

pub fn recv(
    pidptr: usize,
    message_ptr: usize,
    message_max: usize,
    block: usize,
) -> Result<usize, AeroSyscallError> {
    let current = get_scheduler().current_task();

    if block == 0 {
        // nonblocking read
        let mut msgqueue = current.message_queue.queue.lock();
        let item = msgqueue
            .pop_front()
            .expect("empty message queues should always be deleted!");

        if item.data.len() > message_max {
            msgqueue.push_front(item);
            return Err(AeroSyscallError::E2BIG);
        }

        return handle_recieve(pidptr, message_ptr, message_max, item);
    }

    let mq = &current.message_queue;
    let mut our_queue = mq
        .blockqueue
        .block_on(&mq.queue, |msg| msg.front().is_some())
        .unwrap();

    let msg = our_queue
        .pop_front()
        .expect("ipc_recieve: someone else stole our message!");

    if msg.data.len() > message_max {
        our_queue.push_front(msg);
        Err(AeroSyscallError::E2BIG)
    } else {
        handle_recieve(pidptr, message_ptr, message_max, msg)
    }
}

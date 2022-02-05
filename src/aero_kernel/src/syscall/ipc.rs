use crate::{
    userland::{
        scheduler::get_scheduler,
    },
    utils::{
        sync::{BlockQueue, Mutex},
        validate_slice_mut,
    },
};
use aero_syscall::AeroSyscallError;
use alloc::vec::Vec;
use hashbrown::HashMap;
use lazy_static::lazy_static;

struct Message {
    from: usize,
    data: Vec<u8>,
}

lazy_static! {
    static ref BLOCK_QUEUE: BlockQueue = BlockQueue::new();
    static ref MESSAGES: Mutex<HashMap<usize, Option<Message>>> = Mutex::new(HashMap::new());
}

pub fn send(pid: usize, message: usize, messagesiz: usize) -> Result<usize, AeroSyscallError> {
    let payload =
        validate_slice_mut(message as *mut u8, messagesiz).ok_or(AeroSyscallError::EINVAL)?;

    let mut bqueue = BLOCK_QUEUE
        .block_on(&MESSAGES, |msg| {
            let mp = msg.get(&pid);
            match mp {
                Some(None) => true,
                _ => false,
            }
        })
        .unwrap();
    let bqueueitem = bqueue.get_mut(&pid).unwrap();
    bqueueitem.replace(Message {
        from: get_scheduler().current_task().pid().as_usize(),
        data: payload.to_vec(),
    });
    BLOCK_QUEUE.notify_complete();
    Ok(0)
}
pub fn recv(
    pidptr: usize,
    messageptr: usize,
    messagemax: usize,
    block: usize,
) -> Result<usize, AeroSyscallError> {
    let output =
        validate_slice_mut(messageptr as *mut u8, messagemax).ok_or(AeroSyscallError::EINVAL)?;
    let pid = get_scheduler().current_task().pid().as_usize();

    if block == 0 {
        // nonblocking read
        let mut msgqueue = MESSAGES.lock();
        match msgqueue.get(&pid) {
            Some(m) => match m {
                None => return Err(AeroSyscallError::EAGAIN),
                Some(_) => {
                    let m = msgqueue.remove(&pid).unwrap().unwrap();
                    output.split_at_mut(m.data.len()).0.copy_from_slice(&m.data);
                    BLOCK_QUEUE.notify_complete();
                    unsafe {
                        *(pidptr as *mut usize) = m.from;
                    }
                    return Ok(m.data.len());
                }
            },
            None => {
                // just set it up
                msgqueue.insert(pid, None);
                return Err(AeroSyscallError::EAGAIN);
            }
        }
    }

    let mut mqueue = MESSAGES.lock();
    mqueue.insert(pid, None);
    BLOCK_QUEUE.notify_complete();
    drop(mqueue);

    let mut bqueue = BLOCK_QUEUE
        .block_on(&MESSAGES, |msg| {
            let mp = msg.get(&pid);
            match mp {
                Some(Some(_)) => true,
                _ => false,
            }
        })
        .unwrap();
    let m = bqueue.remove(&pid).unwrap().unwrap();
    output.split_at_mut(m.data.len()).0.copy_from_slice(&m.data);
    BLOCK_QUEUE.notify_complete();
    unsafe {
        *(pidptr as *mut usize) = m.from;
    }
    Ok(m.data.len())
}

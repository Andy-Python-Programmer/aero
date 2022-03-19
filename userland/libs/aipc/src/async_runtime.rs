use aero_syscall::{sys_log, sys_getpid};
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
use alloc::{collections::VecDeque, sync::Arc};
use core::{
    cell::RefCell,
    future::Future,
    mem::swap,
    ops::DerefMut,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};
use hashbrown::HashMap;
use spin::{mutex::Mutex, Once};

struct Task {
    future: Pin<Box<dyn Future<Output = ()> + Send>>,
}
impl Task {
    pub fn new(future: impl Future<Output = ()> + Send + 'static) -> Task {
        Task {
            future: Box::pin(future),
        }
    }
    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        self.future.as_mut().poll(context)
    }
}

#[derive(Default)]
pub struct Message {
    pub pid: usize,
    pub message: Vec<u8>,
}

static SHOULD_THAW_ALL: AtomicBool = AtomicBool::new(false);
static MESSAGE_HANDLERS: Mutex<Once<Vec<Box<dyn Fn(Arc<Message>) -> bool + Send>>>> =
    Mutex::new(Once::new());
static RX_ARENA: Mutex<[u8; 0x4000]> = Mutex::new([0; 0x4000]);

static REPLY_TOKENS: Mutex<Once<HashMap<usize, RefCell<Message>>>> = Mutex::new(Once::new());
static REPLY_ID: AtomicUsize = AtomicUsize::new(1);
static NEW_TASK_QUEUE: Mutex<Once<VecDeque<Task>>> = Mutex::new(Once::new());

struct MessageFuture {
    rid: usize,
}
impl Future for MessageFuture {
    type Output = Message;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        // get access to REPLY_TOKENS
        REPLY_TOKENS.lock().call_once(|| HashMap::new());
        let mut reply_tokens = REPLY_TOKENS.lock();
        let reply_tokens = reply_tokens.get_mut().unwrap();
        let reply_token = reply_tokens.get(&self.rid).unwrap();
        let r = reply_token.borrow();
        if r.pid == 0 {
            Poll::Pending
        } else {
            drop(r);
            drop(reply_token);
            let r = reply_tokens.remove(&self.rid).unwrap();
            let t = r.take();
            Poll::Ready(t)
        }
    }
}

pub fn alloc_reply_id() -> (usize, impl Future<Output = Message>) {
    // get a unique reply id
    let rid = REPLY_ID.fetch_add(2, Ordering::SeqCst);

    // get access to REPLY_TOKENS
    REPLY_TOKENS.lock().call_once(|| HashMap::new());
    let mut reply_tokens = REPLY_TOKENS.lock();
    let reply_tokens = reply_tokens.get_mut().unwrap();

    // define a reply token with a dummy message
    reply_tokens.insert(rid, RefCell::new(Message::default()));

    // return the two values
    (rid, MessageFuture { rid })
}
pub fn create_server(srv: Box<dyn Fn(Arc<Message>) -> bool + Send>) {
    let mut handlers = MESSAGE_HANDLERS.lock();
    handlers.call_once(|| Vec::new());
    let handlers = handlers.get_mut().unwrap();
    handlers.push(srv);
}

pub struct AsyncRuntime {
    task_queue: VecDeque<Task>,
    idle_queue: VecDeque<Task>,
}

fn new_raw_waker() -> RawWaker {
    fn no_op(_: *const ()) {}
    fn thaw(_: *const ()) {
        SHOULD_THAW_ALL.store(true, Ordering::SeqCst);
    }
    fn clone(_: *const ()) -> RawWaker {
        new_raw_waker()
    }

    let vtable = &RawWakerVTable::new(clone, thaw, thaw, no_op);
    RawWaker::new(0 as *const (), vtable)
}

pub fn spawn(task: impl Future<Output = ()> + Send + 'static) {
    let mut new_task_queue = NEW_TASK_QUEUE.lock();
    new_task_queue.call_once(|| VecDeque::new());
    let new_task_queue = new_task_queue.get_mut().unwrap();
    new_task_queue.push_back(Task::new(task));
}

pub trait Listener {
    fn listen();
}

impl AsyncRuntime {
    pub fn new() -> AsyncRuntime {
        AsyncRuntime {
            task_queue: VecDeque::new(),
            idle_queue: VecDeque::new(),
        }
    }

    pub fn spawn(&mut self, task: impl Future<Output = ()> + Send + 'static) {
        self.task_queue.push_back(Task::new(task))
    }

    pub fn wake_all(&mut self) {
        assert!(self.task_queue.len() == 0);
        swap(&mut self.task_queue, &mut self.idle_queue);
    }

    pub fn maybe_wake_all(&mut self) {
        if SHOULD_THAW_ALL.swap(false, Ordering::SeqCst) {
            self.wake_all();
        }
    }

    pub fn run(&mut self) {
        // SAFETY: the waker implementation's behaviour does not violate the invariants
        // for RawWakerVTable
        let waker = unsafe { Waker::from_raw(new_raw_waker()) };

        loop {
            let mut progress = true;
            while progress {
                progress = false;

                self.maybe_wake_all();
                {
                    let mut new_task_queue = NEW_TASK_QUEUE.lock();
                    if let Some(new_task_queue) = new_task_queue.get_mut() {
                        while let Some(task) = new_task_queue.pop_front() {
                            self.task_queue.push_back(task);
                            progress = true
                        }
                    }
                }

                // println!();
                while let Some(mut task) = self.task_queue.pop_front() {
                    progress = true;
                    let mut context = Context::from_waker(&waker);
                    match task.poll(&mut context) {
                        Poll::Ready(()) => {} // task done
                        Poll::Pending => self.idle_queue.push_back(task),
                    }
                }
            }

            // all tasks that could complete completed. wait for an IPC message
            let mut pid: usize = 0;
            let mut arena = RX_ARENA.lock();
            match aero_syscall::sys_ipc_recv(&mut pid, arena.deref_mut(), true) {
                Ok(data) => {
                    // move the data to the heap to release the rx arena
                    let d = data.to_vec();
                    drop(data);
                    drop(arena);

                    // sys_log(&format!("[aipc] {}->{} {:02x?}", pid, sys_getpid().unwrap(), d)).unwrap();
                    
                    // have each enrolled message handler handle a message
                    if d.len() < 8 {
                        println!(
                            "[aipc] invalid message from pid {}: too short: {:x?}",
                            pid, d
                        );
                        continue;
                    }
                    let message = Message { pid, message: d };
                    if message.message[0] & 1 == 0 {
                        // server
                        let message = Arc::new(message);
                        let mut message_handlers = MESSAGE_HANDLERS.lock();
                        message_handlers.call_once(|| Vec::new());
                        let message_handlers = message_handlers.get_mut().unwrap();

                        let mut success: bool = false;
                        for m in message_handlers {
                            if m(Arc::clone(&message)) {
                                success = true
                            }
                        }
                        if success {
                            self.wake_all();
                        } else {
                            println!("[aipc] message not handled: {:x?}", message.message);
                        }
                    } else {
                        // client
                        // the unwrap is fine because we cannot fail
                        let id: usize = postcard::from_bytes(&message.message[0..8]).unwrap();
                        let reply_tokens = REPLY_TOKENS.lock();
                        let reply_tokens = reply_tokens.call_once(|| HashMap::new());
                        match reply_tokens.get(&id) {
                            Some(rq) => {
                                let old = rq.borrow();
                                if old.pid != 0 {
                                    println!("[aipc] message reply for request from pid {} delivered multiple times!", pid);
                                    continue;
                                }
                                drop(old);
                                rq.replace(message);
                                self.wake_all();
                            }
                            None => {
                                println!("[aipc] pid {} delivered a reply without a corresponding request!", pid);
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("[aipc] error receiving: {:?}", e);
                }
            }
        }
    }
}

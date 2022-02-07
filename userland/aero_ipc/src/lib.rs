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
#![feature(decl_macro)]

pub extern crate postcard;
pub extern crate serde;

use aero_syscall::{sys_ipc_recv, sys_ipc_send};
use core::ops::DerefMut;
use core::sync::atomic::{AtomicUsize, Ordering};
use lazy_static::lazy_static;
use serde::Deserialize;

/// A MessageHandler is a trait describing an IPC client
pub trait MessageHandler: Send + Sync {
    fn handle(&mut self, src: usize, msg: &[u8]) -> Result<Option<Vec<u8>>, ()>;
}

/// A MessageTransport allows for high-level IPC exchanges over the IPC interfce.
/// It also handles the allocation of request identifiers.
pub trait MessageTransport {
    fn alloc_id() -> usize;
    fn free_id(id: usize);
    fn exchange(meta: usize, mid: usize, data: &[u8]) -> Vec<u8>;
}

/// A SendRecieveTransport transfers messages by using the IPC system calls.
pub struct SendRecieveTransport;

// trust me, this seed is fine
static IDALLOC: AtomicUsize = AtomicUsize::new(0xde73_ce13_600f_e4e9);

impl MessageTransport for SendRecieveTransport {
    fn alloc_id() -> usize {
        let value = IDALLOC.fetch_add(1, Ordering::SeqCst);
        // a small attempt at seed obfuscation
        IDALLOC.fetch_xor(value << 13, Ordering::SeqCst);
        IDALLOC.fetch_xor(value >> 7, Ordering::SeqCst);
        IDALLOC.fetch_xor(value << 17, Ordering::SeqCst);
        return IDALLOC.fetch_add(1, Ordering::SeqCst) >> 3;
    }

    fn free_id(_: usize) {}

    fn exchange(meta: usize, mid: usize, msg: &[u8]) -> Vec<u8> {
        // send the data
        sys_ipc_send(meta, msg).expect("exchange failed: request failed!");
        // now wait for a repsonse
        loop {
            // get a response
            let rx = service_with_response_finding();
            match rx {
                // if we got a response,
                Some((srcpid, mut msg)) => {
                    // and the response has the correct message ID...
                    let mut deser = postcard::Deserializer::from_bytes(&msg);
                    let msgid = usize::deserialize(&mut deser)
                        .expect("message ID not present in the message!");
                    if msgid == (mid << 1) | 1 && meta == srcpid {
                        // return the message contents!
                        return msg.split_off(core::mem::size_of::<usize>());
                    }
                }
                None => {}
            }
        }
    }
}

/// The IPC inteface macro
///
/// You can create interfaces like this:
/// ```no_run
/// aero_ipc::ipc! {
///     trait Hello {
///         fn hello(favorite_number: i32) -> ();
///     }
/// ```
///
/// Then, Hello::Client is the client interface, Hello::Server is the server
/// inteface and Hello::handler instantiates a MessageHandler that can be added
/// to the listening pool.
#[macro_export]
macro_rules! ipc {
    { trait $nm:ident {
        $(
            fn $fnnm:ident($($argname:ident : $argty:ty),*) $(-> $t:ty)?;
        )*
    } } => {
        #[allow(non_snake_case)]
        pub mod $nm {
            use $crate::{postcard, serde};
            pub struct Client<T: $crate::MessageTransport> {
                pub pid: usize,
                pub phantom: ::core::marker::PhantomData<T>,
            }
            impl<T: $crate::MessageTransport> Client<T> {
                pub fn pid(&self) -> usize {
                    self.pid
                }
                $(
                    pub fn $fnnm(&self, $($argname: $argty),*) $(-> $t)? {
                        let mid = T::alloc_id();
                        let msg = postcard::to_allocvec(&(
                            mid<<1, // messageid
                            concat!(stringify!($nm), "::", stringify!($fnnm)) // method
                            $(, $argname)* // args
                        )).expect("serialize failed!");
                        let resp = T::exchange(self.pid, mid, &msg);
                        T::free_id(mid);
                        postcard::from_bytes(&resp).expect("deserialize failed!")
                    }
                )*
            }

            pub fn open(pid: usize) -> Client<$crate::SendRecieveTransport> {
                Client { pid, phantom: ::core::marker::PhantomData{} }
            }

            pub trait Server: Send + Sync {
                $(
                    fn $fnnm(&self, $($argname: $argty)*) $(-> $t)?;
                )*
            }

            struct MessageHandlingProxy<T: 'static + Server>(T);

            pub fn handler<T: 'static + Server>(server: T) -> Box<dyn $crate::MessageHandler> {
                Box::new(MessageHandlingProxy(server))
            }

            impl<T: Server> $crate::MessageHandler for MessageHandlingProxy<T> {
                fn handle(&mut self, _: usize, msg: &[u8]) -> Result<Option<Vec<u8>>, ()> {
                    use serde::Deserialize;

                    let mut deser = postcard::Deserializer::from_bytes(msg);
                    // TODO(pitust): cache this in the recieve part of the handler
                    //? i don't think it would help *that* much though
                    let msgid: usize = usize::deserialize(&mut deser).or_else(|e| {
                        println!("\x1b[31;1merr\x1b[0m message id failed to deserialize!");
                        Err(())
                    })?;

                    let method = String::deserialize(&mut deser).or_else(|e| {
                        println!("\x1b[31;1merr\x1b[0m message name failed to deserialize!");
                        Err(())
                    })?;

                    match method.as_str() {
                        $(
                            concat!(stringify!($nm), "::", stringify!($fnnm)) => {
                                Ok(Some(postcard::to_allocvec(&(msgid|1, self.0.$fnnm(
                                    $(
                                        <$argty>::deserialize(&mut deser).or_else(|e| {
                                            println!("\x1b[31;1merr\x1b[0m message deserialization failed!");
                                            Err(())
                                        })?
                                    ),*
                                ))).expect("reply failed to serialize!")))
                            },
                        )*
                        _ => Ok(None)
                    }
                }
            }
        }
    }
}

lazy_static! {
    static ref HANDLER_LIST: spin::Mutex<Vec<Box<dyn MessageHandler>>> = spin::Mutex::new(vec![]);
    static ref RX_ARENA: spin::Mutex<Box<[u8; 0x4000]>> = spin::Mutex::new(Box::new([0; 0x4000]));
}

/// Register a request listener.
pub fn listen(iface: Box<dyn MessageHandler>) {
    let mut list = HANDLER_LIST
        .try_lock()
        .expect("cannot listen() in a request handler!");

    list.push(iface);
}

/// Handle an IPC request from a specified process.
pub fn handle_request(src: usize, msg: &[u8]) -> Option<Vec<u8>> {
    let mut list = HANDLER_LIST
        .try_lock()
        .expect("cannot nest request handlers!");

    if (msg[0] & 1) == 1 {
        println!(
            "\x1b[32;1mwarn\x1b[0m recieved random response from {}!",
            src
        );
        return None;
    }

    for i in list.deref_mut() {
        match i.handle(src, msg) {
            Ok(Some(data)) => return Some(data),
            Ok(None) => {}
            Err(_) => return None,
        }
    }

    println!(
        "\x1b[32;1mwarn\x1b[0m failed to dispatch message from {}!",
        src
    );

    None
}

fn service_with_response_finding() -> Option<(usize, Vec<u8>)> {
    let mut src: usize = 0;
    let mut arena = RX_ARENA.try_lock().expect("recieve arena is locked!");
    let msg = sys_ipc_recv(&mut src, arena.as_mut(), true).expect("sys_ipc_recv failed!");

    // if it's a response
    if (msg[0] & 1) == 1 {
        return Some((src, msg.to_vec()));
    }

    if let Some(data) = handle_request(src, msg) {
        sys_ipc_send(src, &data).expect("sys_ipc_send failed, reply dropped!");
    }

    None
}

/// Service one request from the IPC queues
pub fn service_request() {
    let mut src: usize = 0;
    let mut arena = RX_ARENA
        .try_lock()
        .expect("service_request: recieve arena is locked!");

    let msg = sys_ipc_recv(&mut src, arena.as_mut(), true).expect("sys_ipc_recv failed!");

    match handle_request(src, msg) {
        Some(data) => {
            sys_ipc_send(src, &data).expect("sys_ipc_send failed, reply dropped!");
        }
        _ => {}
    }
}

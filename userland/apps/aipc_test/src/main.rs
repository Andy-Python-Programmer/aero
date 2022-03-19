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
use core::sync::atomic::{AtomicUsize, Ordering, AtomicBool};

use aero_syscall::{sys_fork, sys_exit};
use aipc::async_runtime::Listener;

#[aipc::def("TestObj")]
trait TestObj {
    async fn create() -> TestObj;
    async fn foo(&self);
    async fn kill(&self);
}
#[aipc::def("ServerControlObj")]
trait ServerControlObj {
    async fn create() -> ServerControlObj;
    async fn can_control(&self) -> bool;
    async fn quit(&self);
}

static COUNTER: AtomicUsize = AtomicUsize::new(1);

struct NoData {
    data: usize,
}

#[aipc::object(TestObjSrv as TestObj)]
impl NoData {
    fn create() -> NoData {
        return NoData {
            data: COUNTER.fetch_add(1, Ordering::SeqCst),
        };
    }
    fn foo(&self) {
        println!("Hello from obj {}!", self.data);
    }
}

static SCS_PERMIT_ONCE: AtomicBool = AtomicBool::new(true);
pub struct ServerControlState {
    permitted: bool
}
#[aipc::object(ServerControlSrv as ServerControlObj)]
impl ServerControlState {
    pub fn create() -> ServerControlState {
        return ServerControlState {
            permitted: SCS_PERMIT_ONCE.swap(false, Ordering::SeqCst)
        };
    }
    pub fn can_control(&self) -> bool {
        self.permitted
    }
    pub fn quit(&self) {
        if self.permitted {
            println!("[ServerControlSrv] exiting!");
            aipc::async_runtime::spawn(async {
                sys_exit(0);
            });
        }
    }
}


fn main() {
    let mut rt = aipc::async_runtime::AsyncRuntime::new();
    rt.spawn(async {
        let pid = sys_fork().unwrap();
        if pid == 0 {
            // server
            ServerControlSrv::listen();
            TestObjSrv::listen();
        } else {
            // client
            let ctl = ServerControlObj::create(pid).await;
            let o = TestObj::create(pid).await;
            o.foo().await;
            ctl.quit().await;
            sys_exit(0);
        }
    });
    rt.run();
}

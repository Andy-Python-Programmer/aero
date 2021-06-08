/*
 * Copyright 2021 The Aero Project Developers. See the COPYRIGHT
 * file at the top-level directory of this project.
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 * <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
 * option. This file may not be copied, modified, or distributed
 * except according to those terms.
 */

use crate::userland::scheduler;

pub fn exit(status: usize) -> ! {
    log::trace!("Exiting the current process with status: {}", status);
    scheduler::exit_current_process(status);
}

pub fn shutdown() -> ! {
    crate::fs::cache::clear_inode_cache();
    // TODO
    loop {}
}

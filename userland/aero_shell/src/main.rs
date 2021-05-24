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

static TEST_BSS_NON_ZERO: usize = usize::MAX;
static TEST_BSS_ZEROED: usize = 0x00;

fn main() {
    {
        assert!(TEST_BSS_ZEROED == 0x00);
        assert!(TEST_BSS_NON_ZERO == usize::MAX);
    }

    aero_syscall::sys_open("/dev/stdout", 0x00);
    aero_syscall::sys_write(1, "Hello, World".as_bytes());
}

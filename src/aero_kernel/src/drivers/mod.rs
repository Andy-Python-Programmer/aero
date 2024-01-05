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

#[cfg(target_arch = "x86_64")]
pub mod block;
#[cfg(target_arch = "x86_64")]
pub mod drm;
// FIXME: aarch64 port
#[cfg(target_arch = "x86_64")]
pub mod keyboard;
// FIXME: aarch64 port
#[cfg(target_arch = "x86_64")]
pub mod lai;
// FIXME: aarch64 port
pub mod e1000;
// #[cfg(feature = "gdbstub")]
pub mod gdbstub;
pub mod mouse;
#[cfg(target_arch = "x86_64")]
pub mod pci;
pub mod pty;
pub mod tty;

cfg_match! {
    cfg(target_arch = "x86_64") => {
        pub mod uart_16550;
        pub use self::uart_16550 as uart;
    }

    cfg(target_arch = "aarch64") => {
        pub mod uart_pl011;
        pub use self::uart_pl011 as uart;
    }
}

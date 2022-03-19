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
pub mod system_server {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub enum Error {
        AlreadyProvided,
        NotFound,
    }

    #[aipc::def("SystemServer")]
    pub trait SystemServer {
        fn open() -> SystemServer;
        fn announce(&self, pid: usize, name: &str) -> Result<(), Error>;
        fn discover(&self, name: &str) -> Result<usize, Error>;
    }
}
pub mod window_server {
    #[aipc::def("WindowServer")]
    pub trait WindowServer {
        fn open() -> WindowServer;
        fn create_window(&self, name: &str) -> usize;
    }
}

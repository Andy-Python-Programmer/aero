use crate::ipc;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum SystemServiceError {
    AlreadyProvided,
    NotFound,
}

pub type SystemServiceResult<T> = Result<T, SystemServiceError>;

ipc! {
    trait SystemService {
        fn announce(pid: usize, name: &str) -> crate::SystemServiceResult<()>;
        fn discover(name: &str) -> crate::SystemServiceResult<usize>;
    }
}

ipc! {
    trait WindowService {
        fn create_window(name: &str) -> usize;
    }
}

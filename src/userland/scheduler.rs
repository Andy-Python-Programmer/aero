use lazy_static::lazy_static;

lazy_static! {
    pub static ref SCHEDULER: Scheduler = Scheduler::new();
}

#[derive(Debug)]
pub struct Scheduler {}

impl Scheduler {
    #[inline]
    const fn new() -> Self {
        Self {}
    }
}

unsafe impl Send for Scheduler {}
unsafe impl Sync for Scheduler {}

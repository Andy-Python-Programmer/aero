use crate::userland::scheduler;

pub fn exit(status: usize) {
    log::debug!("Exiting the current process with status: {}", status);

    scheduler::get_scheduler();
}

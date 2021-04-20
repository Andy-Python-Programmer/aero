use crate::userland::scheduler;

pub fn exit() {
    scheduler::get_scheduler();
}

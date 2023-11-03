// TODO: Is it worth adding a GDB stub to facilitate userland debugging?
pub fn init() {}

crate::module_init!(init, ModuleType::Other);

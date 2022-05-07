use alloc::sync::Arc;
use spin::Once;

/// ## Reference
/// * [ACPI Sleeping States](https://uefi.org/specs/ACPI/6.4/16_Waking_and_Sleeping/sleeping-states.html)
#[repr(u8)]
pub enum SleepState {
    S5 = 5,
}

pub trait AmlSubsystem: Send + Sync {
    fn enter_state(&self, state: SleepState);
}

static AML_SUBSYSTEM: Once<Arc<dyn AmlSubsystem>> = Once::new();

pub fn get_subsystem() -> Arc<dyn AmlSubsystem> {
    AML_SUBSYSTEM.get().unwrap().clone()
}

pub fn init(subsystem: Arc<dyn AmlSubsystem>) {
    assert!(
        AML_SUBSYSTEM.get().is_none(),
        "aml: subsystem already initialized"
    );

    AML_SUBSYSTEM.call_once(|| subsystem);
    log::debug!("aml: subsystem initialized");
}

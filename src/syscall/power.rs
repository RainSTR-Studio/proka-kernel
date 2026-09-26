//! The power action in syscall.
//!
//! Registered as syscall 1.
use crate::{
    acpi::power::{poweroff, reboot},
    scheduler::{DRIVER_QUEUE, NORMAL_QUEUE},
};
use num_enum::TryFromPrimitive;

/// The power actions.
#[derive(Debug, PartialEq, Eq, TryFromPrimitive)]
#[repr(u64)]
pub enum PowerActions {
    /// The power action to poweroff the whole machine.
    PowerOff = 0,

    /// The power action which makes this machine reset (reboot).
    Reboot = 1,
}

/// The power action syscall entry.
pub extern "C" fn power(power_action: u64, _: u64, _: u64, _: u64, _: u64) -> i64 {
    unsafe { core::arch::asm!("cli") } // Avoid scheduler switch tasks
    let Ok(action) = PowerActions::try_from(power_action) else {
        return -2;
    };

    // Kill all tasks...
    DRIVER_QUEUE.lock().clear();
    NORMAL_QUEUE.lock().clear();

    // Match actions...
    match action {
        PowerActions::PowerOff => poweroff(),
        PowerActions::Reboot => reboot(),
    }
}

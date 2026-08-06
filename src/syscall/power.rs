//! The power action in syscall.
//!
//! Registered as syscall 1.
use crate::{
    acpi::power::{poweroff, reboot},
    scheduler::{DRIVER_QUEUE, NORMAL_QUEUE},
};

/// The power actions.
#[repr(C)]
pub enum PowerActions {
    /// The power action to poweroff the whole machine.
    PowerOff,

    /// The power action which makes this machine reset (reboot).
    Reboot,
}

impl PowerActions {
    /// Convert to this action from u64.
    #[inline]
    pub fn from_u64(action: u64) -> Self {
        match action {
            0 => Self::PowerOff,
            1 => Self::Reboot,
            _ => panic!("Invalid power action: {}", action),
        }
    }
}

/// The power action syscall entry.
pub extern "C" fn power(power_action: u64, _: u64, _: u64, _: u64, _: u64) -> i64 {
    unsafe { core::arch::asm!("cli") } // Avoid scheduler switch tasks
    let action = PowerActions::from_u64(power_action);

    // Kill all tasks...
    DRIVER_QUEUE.lock().clear();
    NORMAL_QUEUE.lock().clear();

    // Match actions...
    match action {
        PowerActions::PowerOff => poweroff(),
        PowerActions::Reboot => reboot(),
    }
}

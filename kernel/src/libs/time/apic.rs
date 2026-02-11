use log::info;

pub const XAPIC_LVT_TIMER_OFFSET: u32 = 0x320;
pub const XAPIC_TIMER_INIT_COUNT_OFFSET: u32 = 0x380;
pub const XAPIC_TIMER_CUR_COUNT_OFFSET: u32 = 0x390;
pub const XAPIC_TIMER_DIV_CONF_OFFSET: u32 = 0x3E0;

/// Calibrate the APIC timer using PIT.
///
/// # Arguments
/// * `read_reg` - A closure to read an APIC register.
/// * `write_reg` - A closure to write an APIC register.
/// * `timer_vector` - The interrupt vector to use for the timer.
///
/// # Returns
/// The number of ticks per 10ms.
pub unsafe fn calibrate_timer<F1, F2>(read_reg: F1, write_reg: F2, timer_vector: u8) -> u64
where
    F1: Fn(u32) -> u32,
    F2: Fn(u32, u32),
{
    // Stop timer
    write_reg(XAPIC_TIMER_INIT_COUNT_OFFSET, 0);
    // Set divider to 16
    write_reg(XAPIC_TIMER_DIV_CONF_OFFSET, 0x3);

    let (ticks_per_10ms, _) = super::calibrate_with_pit(10000, || {
        // Set APIC timer to max
        write_reg(XAPIC_TIMER_INIT_COUNT_OFFSET, 0xFFFFFFFF);
        move || {
            let current_count = read_reg(XAPIC_TIMER_CUR_COUNT_OFFSET);
            0xFFFFFFFF - current_count as u64
        }
    });

    info!("APIC Timer calibrated: {} ticks per 10ms", ticks_per_10ms);

    // Set timer for periodic interrupt at 100Hz (10ms)
    write_reg(XAPIC_LVT_TIMER_OFFSET, 0x20000 | timer_vector as u32); // Periodic mode
    write_reg(XAPIC_TIMER_INIT_COUNT_OFFSET, ticks_per_10ms as u32);

    ticks_per_10ms
}

//! CPU control primitives (x86_64).
//!
//! Small architecture-specific helpers for interrupt handling and halt.

/// Enable interrupts on the current CPU.
pub fn enable_interrupts() {
    x86_64::instructions::interrupts::enable();
}

/// Halt the current CPU until the next interrupt.
pub fn hlt() {
    x86_64::instructions::hlt();
}

/// Exit QEMU via the debug-exit port (0xf4). Used by the test runner.
pub fn exit_qemu(exit_code: u32) {
    use x86_64::instructions::port::Port;
    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code);
    }
}

//! APIC handler
use crate::apic::eoi;
use x86_64::structures::idt::InterruptStackFrame;

// Error handler for LAPIC
#[unsafe(link_section = ".gdata")]
pub extern "x86-interrupt" fn error(_stack_frame: InterruptStackFrame) {
    // This need an EOI
    eoi();
}

// Spurious handler for LAPIC
#[unsafe(link_section = ".gdata")]
pub extern "x86-interrupt" fn spurious(_stack_frame: InterruptStackFrame) {
    // Fake interrupt doesn't need EOI
    // Just return
}
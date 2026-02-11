pub mod apic;
pub mod gdt;
pub mod handler;
pub mod idt;
pub mod pic;

use crate::interrupts::apic::registry::{IrqHandler, IRQ_REGISTRY};
use crate::interrupts::idt::IRQ_BASE;

/// Request an IRQ and register its handler.
/// This handles both I/O APIC routing and software registry.
pub fn request_irq(irq_num: u8, name: &'static str, handler: IrqHandler) {
    let vector = IRQ_BASE + irq_num;

    // 1. Route hardware IRQ to vector
    apic::ioapic::route_irq(irq_num, vector, 0);

    // 2. Register software handler
    IRQ_REGISTRY
        .lock()
        .register(vector, name, handler)
        .expect("Failed to register IRQ handler");
}

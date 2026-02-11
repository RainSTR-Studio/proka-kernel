use crate::libs::time::time_since_boot;
use core::sync::atomic::{AtomicU64, Ordering};
pub use spin::RwLock;
use x86_64::structures::idt::InterruptStackFrame;

pub static IRQ_REGISTRY: RwLock<IrqRegistry> = RwLock::new(IrqRegistry::new());

pub type IrqHandler = for<'a> fn(IrqContext<'a>) -> IrqResult;

pub struct IrqRegistry {
    handlers: [Option<IrqHandler>; 256],
    names: [Option<&'static str>; 256],
    stats: [IrqStats; 256],
}

pub struct IrqContext<'a> {
    pub vector: u8,
    pub irq_number: Option<u8>, // None for exceptions
    pub stack_frame: &'a InterruptStackFrame,
    pub error_code: Option<u64>,
}

pub enum IrqResult {
    /// Continue execution
    Continue,
    /// Interrupt is handled
    Handled,
}

pub struct IrqStats {
    /// Number of interrupts
    pub count: AtomicU64,
    /// Time spent in last interrupt handler
    pub last_time: AtomicU64,
    /// Time of last interrupt
    pub interrupt_time: AtomicU64,
}

impl IrqStats {
    pub const fn new() -> Self {
        Self {
            count: AtomicU64::new(0),
            last_time: AtomicU64::new(0),
            interrupt_time: AtomicU64::new(0),
        }
    }
}

macro_rules! array_256 {
    ($val:expr) => {
        [
            $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val,
            $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val,
            $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val,
            $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val,
            $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val,
            $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val,
            $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val,
            $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val,
            $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val,
            $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val,
            $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val,
            $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val,
            $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val,
            $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val,
            $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val,
            $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val,
            $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val,
            $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val, $val,
            $val, $val, $val, $val,
        ]
    };
}

impl IrqRegistry {
    pub const fn new() -> Self {
        Self {
            handlers: [None; 256],
            names: [None; 256],
            stats: array_256!(IrqStats::new()),
        }
    }

    pub fn register(
        &mut self,
        vector: u8,
        name: &'static str,
        handler: IrqHandler,
    ) -> Result<(), &'static str> {
        if self.handlers[vector as usize].is_some() {
            return Err("Handler already registered");
        }
        self.handlers[vector as usize] = Some(handler);
        self.names[vector as usize] = Some(name);
        let stat = &self.stats[vector as usize];
        stat.count.store(0, Ordering::Relaxed);
        stat.last_time.store(0, Ordering::Relaxed);
        stat.interrupt_time.store(0, Ordering::Relaxed);
        Ok(())
    }

    pub fn unregister(&mut self, vector: u8) -> Result<(), &'static str> {
        if self.handlers[vector as usize].is_none() {
            return Err("Handler not registered");
        }
        self.handlers[vector as usize] = None;
        self.names[vector as usize] = None;
        Ok(())
    }

    pub fn handle(&self, context: IrqContext) -> IrqResult {
        let vector = context.vector as usize;
        let stat = &self.stats[vector];
        stat.count.fetch_add(1, Ordering::Relaxed);
        let st = time_since_boot();
        stat.interrupt_time.store(st as u64, Ordering::Relaxed);
        match self.handlers[vector] {
            Some(handler) => {
                let result = handler(context);
                stat.last_time
                    .store((time_since_boot() - st) as u64, Ordering::Relaxed);
                result
            }
            None => IrqResult::Continue,
        }
    }

    pub fn get_stats(&self, vector: u8) -> (u64, u64, u64) {
        let stat = &self.stats[vector as usize];
        (
            stat.count.load(Ordering::Relaxed),
            stat.last_time.load(Ordering::Relaxed),
            stat.interrupt_time.load(Ordering::Relaxed),
        )
    }
}

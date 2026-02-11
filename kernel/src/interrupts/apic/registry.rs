use crate::libs::time::time_since_boot;
use spin::Mutex;
use x86_64::structures::idt::InterruptStackFrame;

pub static IRQ_REGISTRY: Mutex<IrqRegistry> = Mutex::new(IrqRegistry::new());

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

#[derive(Default, Clone, Copy, Debug)]
pub struct IrqStats {
    /// Number of interrupts
    pub count: u64,
    /// Time spent in last interrupt handler
    pub last_time: u64,
    /// Time of last interrupt
    pub interrupt_time: u64,
}

impl IrqStats {
    pub const fn new() -> Self {
        Self {
            count: 0,
            last_time: 0,
            interrupt_time: 0,
        }
    }
}

impl IrqRegistry {
    pub const fn new() -> Self {
        Self {
            handlers: [None; 256],
            names: [None; 256],
            stats: [IrqStats::new(); 256],
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
        self.stats[vector as usize] = IrqStats::new();
        Ok(())
    }

    pub fn unregister(&mut self, vector: u8) -> Result<(), &'static str> {
        if self.handlers[vector as usize].is_none() {
            return Err("Handler not registered");
        }
        self.handlers[vector as usize] = None;
        self.names[vector as usize] = None;
        self.stats[vector as usize] = IrqStats::new();
        Ok(())
    }

    pub fn handle(&mut self, context: IrqContext) -> IrqResult {
        let vector = context.vector as usize;
        self.stats[vector].count += 1;
        let st = time_since_boot();
        self.stats[vector].interrupt_time = st as u64;
        match self.handlers[vector] {
            Some(handler) => {
                let result = handler(context);
                self.stats[vector].last_time = (time_since_boot() - st) as u64;
                result
            }
            None => IrqResult::Continue,
        }
    }

    pub fn get_stats(&self, vector: u8) -> IrqStats {
        self.stats[vector as usize]
    }
}

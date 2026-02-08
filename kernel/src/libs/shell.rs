use alloc::{string::String, sync::Arc};

use crate::{
    drivers::{device, Device},
    print, println,
};

pub struct Shell {}
impl Shell {
    pub fn new() -> Self {
        Shell {}
    }

    fn get_device(&self, device: &str) -> Option<Arc<dyn device::CharDevice>> {
        let device_manager = crate::drivers::DEVICE_MANAGER.read();
        if let Some(device) = device_manager.get_device(device) {
            Some(device.as_char_device().expect("non-char device").clone())
        } else {
            device_manager
                .get_device("keyboard")
                .and_then(|dev| dev.as_char_device().cloned())
        }
    }

    fn handle_command(&self, command: &str) {
        match command {
            "help" => {
                println!("Available commands:");
                println!("  help - Show this help message");
                println!("  clear - Clear the screen");
                println!("  exit - Exit the shell");
            }
            "clear" => {
                print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
            }
            "exit" => {
                print!("goodbye!\n");
                // TODO: implement exit functionality
            }
            _ => {
                // ignore empty input, but report unknown commands
                if !command.trim().is_empty() {
                    println!("Unknown command: {}", command.trim());
                }
            }
        }
    }

    pub fn run(&self, device: &str) {
        let device = if let Some(device) = self.get_device(device) {
            device
        } else {
            panic!("WTF??? Where is your keyboard???")
        };
        println!("Welcome to ProkaOS shell!");
        let mut command = String::new();
        let mut buf = [0u8; 1];
        let mut need_prompt = true;
        loop {
            if need_prompt {
                print!("> ");
                need_prompt = false;
            }
            match device.read(&mut buf) {
                Ok(count) if count > 0 => {
                    let c = buf[0] as char;
                    match c {
                        '\x08' | '\x7f' => {
                            if !command.is_empty() {
                                command.pop();
                                print!("{}", '\x08');
                            }
                        }
                        '\n' | '\r' => {
                            print!("\n");
                            self.handle_command(command.trim());
                            command.clear();
                            need_prompt = true;
                        }
                        _ => {
                            print!("{}", c);
                            command.push(c);
                        }
                    }
                }
                Ok(_) => {
                    // 没有数据，暂停 CPU 等待中断
                    x86_64::instructions::hlt();
                }
                Err(_) => {
                    // 读取错误，暂停 CPU
                    x86_64::instructions::hlt();
                }
            }
        }
    }
}

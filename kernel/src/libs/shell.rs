use alloc::{string::String, sync::Arc, vec::Vec};

use crate::{drivers::device, memory::FRAME_ALLOCATOR, print, println};

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
        let cmd = command.split_whitespace().collect::<Vec<&str>>();
        if cmd.is_empty() {
            return;
        }
        let command = cmd[0];
        let args = &cmd[1..];
        match command {
            "help" => {
                println!("Available commands:");
                println!("  help         - Show this help message");
                println!("  ls           - List directory contents");
                println!("  cat <file>   - Show file contents");
                println!("  uptime       - Show system uptime");
                println!("  clear        - Clear the screen");
                println!("  panic        - Trigger a kernel panic");
                println!("  fault        - Trigger a CPU exception (Page Fault)");
                println!("  div0         - Trigger a Divide By Zero exception");
                println!("  memory_stats - Show memory statistics");
                println!("  reboot       - Reboot the system");
                println!("  shutdown     - Shutdown the system");
                println!("  exit         - Exit the shell");
            }
            "clear" => {
                print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
            }
            "exit" => {
                print!("goodbye!\n");
                // TODO: implement exit functionality
            }
            "panic" => {
                panic!("Panic test");
            }
            "fault" => {
                println!("Triggering Page Fault...");
                let ptr = core::ptr::null_mut::<u64>();
                unsafe {
                    *ptr = 0xDEADBEEF;
                }
            }
            "div0" => {
                println!("Triggering Divide By Zero...");
                unsafe {
                    core::arch::asm!("xor rdx, rdx", "mov rax, 0x1234", "xor rcx, rcx", "div rcx",);
                }
            }
            "reboot" => {
                println!("Rebooting...");
                crate::libs::system::reboot();
            }
            "shutdown" => {
                println!("Shutting down...");
                crate::libs::system::shutdown();
            }
            "memory_stats" => {
                crate::memory::paging::print_memory_stats(&FRAME_ALLOCATOR);
            }
            "ls" => {
                let path = if args.is_empty() { "/" } else { args[0] };
                match crate::fs::vfs::VFS.lookup(path) {
                    Ok(inode) => {
                        if inode.node_type() == crate::fs::vfs::VNodeType::Dir {
                            match inode.list() {
                                Ok(entries) => {
                                    for entry in entries {
                                        println!("{}", entry);
                                    }
                                }
                                Err(e) => println!("ls: cannot list directory '{}': {:?}", path, e),
                            }
                        } else {
                            println!("{}", path);
                        }
                    }
                    Err(e) => println!("ls: cannot access '{}': {:?}", path, e),
                }
            }
            "cat" => {
                if args.is_empty() {
                    println!("Usage: cat <file>");
                } else {
                    let path = args[0];
                    match crate::fs::vfs::VFS.open(path) {
                        Ok(file) => match file.read_all() {
                            Ok(content) => {
                                let s = String::from_utf8_lossy(&content);
                                print!("{}", s);
                                if !s.ends_with('\n') {
                                    println!();
                                }
                            }
                            Err(e) => println!("cat: error reading '{}': {:?}", path, e),
                        },
                        Err(e) => println!("cat: cannot open '{}': {:?}", path, e),
                    }
                }
            }
            "uptime" => {
                let total_seconds = crate::libs::time::time_since_boot();
                let hours = (total_seconds / 3600.0) as u64;
                let minutes = ((total_seconds / 60.0) as u64) % 60;
                let seconds = (total_seconds as u64) % 60;
                let seconds_int = total_seconds as u64;
                let ms = ((total_seconds - seconds_int as f64) * 1000.0) as u64;

                if hours > 0 {
                    println!(
                        "up {} hours, {} minutes, {}.{:03} seconds",
                        hours, minutes, seconds, ms
                    );
                } else if minutes > 0 {
                    println!("up {} minutes, {}.{:03} seconds", minutes, seconds, ms);
                } else {
                    println!("up {}.{:03} seconds", seconds, ms);
                }
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
                    x86_64::instructions::hlt();
                }
                Err(_) => {
                    // Read error, pause CPU
                    x86_64::instructions::hlt();
                }
            }
        }
    }
}

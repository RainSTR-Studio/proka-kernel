//! # Proka Kernel - A kernel for ProkaOS
//! Copyright (C) RainSTR Studio 2025, All rights reserved.
//!
//! This provides the test trait and runner.

use crate::{serial_print, serial_println};
use core::arch::asm;
use spin::Mutex;

/// The Jump Buffer to save the CPU state.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct JumpBuffer {
    rbx: u64,
    rsp: u64,
    rbp: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    rip: u64,
}

static TEST_JMP_BUF: Mutex<Option<JumpBuffer>> = Mutex::new(None);
static FAIL_COUNT: Mutex<usize> = Mutex::new(0);

/// Save the current context into the jump buffer.
/// Returns 0 when saving, and 1 when returning from long_jmp.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn set_jmp() -> u64 {
    let mut jmp_buf = JumpBuffer::default();
    let res: u64;

    asm!(
        // 保存寄存器状态
        "mov [rcx + 0], rbx",
        "mov [rcx + 8], rsp",
        "mov [rcx + 16], rbp",
        "mov [rcx + 24], r12",
        "mov [rcx + 32], r13",
        "mov [rcx + 40], r14",
        "mov [rcx + 48], r15",
        // 保存返回地址
        "lea rdx, [rip + 2f]",
        "mov [rcx + 56], rdx",
        // 首次调用返回 0
        "mov rax, 0",
        "jmp 3f",
        // longjmp 返回点
        "2:",
        // longjmp 后返回 1
        "mov rax, 1",
        "3:",
        in("rcx") &mut jmp_buf,
        out("rax") res,
        out("rdx") _,
        clobber_abi("sysv64")
    );

    if res == 0 {
        *TEST_JMP_BUF.lock() = Some(jmp_buf);
    }
    res
}

/// Restore the context and jump back to the set_jmp location.
pub fn long_jmp() -> ! {
    let jmp_buf = TEST_JMP_BUF.lock().expect("No jump buffer set!");

    // 在恢复寄存器前增加错误计数
    // 这样错误计数不会因为寄存器恢复而被覆盖
    let mut fail_count = FAIL_COUNT.lock();
    *fail_count += 1;
    drop(fail_count); // 释放锁，防止死锁

    unsafe {
        asm!(
            // 恢复寄存器状态
            "mov rbx, [rcx + 0]",
            "mov rsp, [rcx + 8]",
            "mov rbp, [rcx + 16]",
            "mov r12, [rcx + 24]",
            "mov r13, [rcx + 32]",
            "mov r14, [rcx + 40]",
            "mov r15, [rcx + 48]",
            // 跳转回保存的地址
            "jmp [rcx + 56]",
            in("rcx") &jmp_buf,
            options(noreturn)
        );
    }
}

/// The trait that assign the function is testable.
pub trait Testable {
    /// The things will run
    fn run(&self) -> ();
}

// This is the default implementation of this trait
impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("Testing {}... ", core::any::type_name::<T>());

        // 使用 setjmp/longjmp 进行测试
        unsafe {
            if set_jmp() == 0 {
                // 第一次执行测试
                self();
                serial_println!("[OK]");
            } else {
                // longjmp 返回，测试失败
                // [FAILED] 已在 panic 处理程序中打印
            }
        }
    }
}

/// This is the test runner, which will run if the test begins.
pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());

    for test in tests {
        test.run();
    }

    let final_fail_count = *FAIL_COUNT.lock();
    serial_println!("Total failures: {}", final_fail_count);

    if final_fail_count > 0 {
        serial_println!("\n[DONE] FAILED: {} tests failed", final_fail_count);
        exit_qemu(QemuExitCode::Failed);
    } else {
        serial_println!("\n[DONE] SUCCESS: All tests passed!");
        exit_qemu(QemuExitCode::Success);
    }
}

/// This is the QEMU exit code
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

/// The function to quit the QEMU
pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}

// The kernel entry, which will start up the test
#[cfg(test)]
#[unsafe(no_mangle)]
pub extern "C" fn kernel_main() -> ! {
    crate::memory::init(); // Initialize memory management
    crate::drivers::init_devices(); // Initialize devices
    crate::libs::time::init(); // Init time system
    crate::libs::logger::init_logger(); // Init log system
    crate::libs::initrd::load_initrd(); // Load initrd
    crate::interrupts::gdt::init(); // Initialize GDT
    crate::interrupts::idt::init_idt(); // Initialize IDT
    crate::interrupts::pic::init(); // Initialize PI
    x86_64::instructions::interrupts::enable(); // Enable interrupts
    crate::test_main();
    loop {}
}

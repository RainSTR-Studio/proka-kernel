//! The power system which is based on ACPI.
extern crate alloc;
use super::ACPI_PLATFORM;
use crate::{acpi::AMLINT, tables::idt::IDT_EMPTY};
use acpi::aml::object::Object::Package;
use acpi::registers::Pm1ControlBit;
use acpi::sdt::fadt::Fadt;
use acpi::{address::AddressSpace, aml::namespace::AmlName};
use alloc::vec::Vec;
use core::str::FromStr;
use log::{debug, warn};
use spin::Lazy;
use x86_64::instructions::port::Port;

/// The FADT table.
static FADT: Lazy<Fadt> = Lazy::new(|| {
    let fadt = ACPI_PLATFORM.tables.find_table::<Fadt>().unwrap();
    *fadt
});

/// Reboot function.
pub fn reboot() -> ! {
    // Hard reboot (Use port)...
    let hard_reboot = || -> ! {
        // For unexpected situations, it will use this.
        warn!(
            "Failed to use ACPI to perform reboot, will use old port method to trigger hard reboot..."
        );

        // Port consts
        const KBD_PORT: u16 = 0x64;
        const KBD_RESET: u8 = 0xFE;

        unsafe {
            let value = Port::<u8>::new(KBD_PORT).read();
            while (value & 0x02) != 0 {
                Port::<u8>::new(KBD_PORT).write(KBD_RESET);
            }
        }

        // Commonly, the PC has shut down.
        // But if CPU still at here, we shall cause triple fault...
        warn!("Port force reboot failed, have to use triple fault...");
        IDT_EMPTY.load();
        unsafe { core::arch::asm!("int3", options(noreturn)) }
    };

    // Use ACPI reboot method first...
    // Get FADT's reset value
    let reg = FADT.reset_register().map_err(|_| hard_reboot()).unwrap(); // Won't panic!
    let val = FADT.reset_value;

    // Check: Is value invalid
    if reg.address == 0 || reg.bit_width != 8 {
        hard_reboot();
    }

    match reg.address_space {
        AddressSpace::SystemIo => {
            debug!("Using port method...");

            // Convert and write port
            if let Ok(port) = u16::try_from(reg.address) {
                unsafe { Port::<u8>::new(port).write(val) };
                loop {}
            }
        }
        AddressSpace::SystemMemory => {
            debug!("Using MMIO method...");

            // Just write bytes
            let mmio_ptr = reg.address as *mut u8;
            unsafe { core::ptr::write(mmio_ptr, val) };
            loop {}
        }
        _ => (),
    }

    loop {}
}

/// The poweroff function.
pub fn poweroff() -> ! {
    // If not work, we have to use port method...
    let hard_poweroff = || -> ! {
        warn!(
            "Failed to use ACPI to perform poweroff, will use old port method to trigger hard poweroff..."
        );
        unsafe {
            core::arch::asm!("out dx, ax", in("dx")(0x604), in("ax")(0x2000), options(noreturn));
        }
    };

    // First, we shall get `\_S5` object from AML interpreter.
    let path = AmlName::from_str("\\_S5").unwrap();
    let s5_wrapped = AMLINT
        .evaluate(path, Vec::new())
        .expect("Failed to get S5 object");
    let s5 = (*s5_wrapped.clone()).clone();

    // The returned object is a package, and the first element is the sleep type.
    // For the second one, it's for pm1b;
    let slp_type = if let Package(package) = s5 {
        // Check: Is package valid?
        if package.len() < 2 {
            warn!("S5 package is not valid (length: {})", package.len());
            hard_poweroff();
        }

        let typea_obj = (*package[0].clone()).clone();
        let typea = if let acpi::aml::object::Object::Integer(i) = typea_obj {
            i
        } else {
            warn!("S5 sleep type is not an integer");
            hard_poweroff();
        };

        u8::try_from(typea).unwrap_or(0)
    } else {
        warn!("S5 object is not a package");
        hard_poweroff();
    };

    // Next is to write the sleep type to PM1a_CNT and PM1b_CNT.
    // For PM1a_CNT, it's required. But for PM1b_CNT, it's optional.
    let pm1 = &ACPI_PLATFORM.registers.pm1_control_registers;
    pm1.set_sleep_typ(slp_type).map_err(|_| hard_poweroff());
    pm1.set_bit(Pm1ControlBit::SleepEnable, true)
        .map_err(|_| hard_poweroff());

    loop {}
}

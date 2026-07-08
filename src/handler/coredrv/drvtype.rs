//! The driver type call.
//!
//! For this module, we have the main function called [`driver_type_reg`], which will
//! done the registration of current's driver type.
//!
//! Also, the args of this call is required:
//!  - arg1: The main type of this driver. See [`DrvType`] for more info;
//!  - arg2: The subtype, which is the pointer of `&str` within 16 bytes length.
extern crate alloc;
use alloc::vec::Vec;
use spin::{Lazy, RwLock};

/// The driver type index.
pub static DRVTYPE_INDEX: Lazy<RwLock<Vec<DrvTypeTable>>> = Lazy::new(|| {
    let table = Vec::new();
    RwLock::new(table)
});

/// The driver type table.
#[derive(Debug, Clone)]
pub struct DrvTypeTable {
    /// The ID of this driver.
    pub id: u16,

    /// The type of this driver.
    pub typ: DrvType,
}

/// The type of coredrv.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum DrvType {
    /// Graphics driver.
    Graphics,

    /// Invalid driver type.
    Invalid,
}

impl DrvType {
    pub fn from_u64(arg1: u64) -> Self {
        match arg1 {
            1 => Self::Graphics,
            _ => Self::Invalid,
        }
    }
}

pub fn driver_type_reg(arg1: u64, _arg2: u64, did: u16) {
    let typ = DrvType::from_u64(arg1);

    // Check: is type invalid?
    if typ == DrvType::Invalid {
        return;
    }

    // Update index...
    let obj = DrvTypeTable { id: did, typ };
    DRVTYPE_INDEX.write().push(obj);

    // TODO: Map the specified MMIO for this driver...
}

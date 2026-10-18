//! Coredrv's drvtype part generic type definitions.
extern crate alloc;
use alloc::vec::Vec;
use spin::{RwLock, LazyLock};
use num_enum::FromPrimitive;

/// The driver type index.
pub static DRVTYPE_INDEX: LazyLock<RwLock<Vec<DrvTypeTable>>> = LazyLock::new(|| {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
#[non_exhaustive]
#[repr(u64)]
pub enum DrvType {
    /// Graphics driver.
    Graphics = 0,

    /// Invalid driver type.
    #[num_enum(default)]
    Invalid = u64::MAX,
}


impl DrvType {
    /// Create this from base class (u8).
    pub fn from_base_class(base_class: u8) -> Self {
        match base_class {
            0x03 => Self::Graphics,
            _ => Self::Invalid,
        }
    }
}

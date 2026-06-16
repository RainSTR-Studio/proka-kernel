//! The driver type call.
//! 
//! For this module, we have the main function called [`driver_type_reg`], which will
//! done the registration of current's driver type.
//!
//! Also, the args of this call is required:
//!  - arg1: The main type of this driver. See [`DrvType`] for more info;
//!  - arg2: The subtype, which is the pointer of `&str` within 16 bytes length.

/// The type of coredrv.
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

pub fn driver_type_reg(arg1: u64, _arg2: u64) {
    let typ = DrvType::from_u64(arg1);

    match typ {
        DrvType::Graphics => {},
        DrvType::Invalid => return,
    }
}

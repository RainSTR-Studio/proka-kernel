//! DMS specified part.
pub mod drvtype;
use num_enum::FromPrimitive;

/// The call number enums of the driver-call half
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
pub enum Callnum {
    /// Register the driver type.
    RegDriverType = 1,

    // TODO: Add more callnum for the driver-call half.
    /// Invalid call num.
    #[num_enum(default)]
    Invalid = u64::MAX,
}

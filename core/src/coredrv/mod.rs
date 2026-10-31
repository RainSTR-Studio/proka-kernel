//! CoreDrv call type definitions.
pub mod drvtype;
use num_enum::FromPrimitive;

/// The call_num enums of coredrv.
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
pub enum Callnum {
    /// Register the driver type.
    RegDriverType = 1,

    // TODO: Add more callnum for coredrv.
    /// Invalid call num.
    #[num_enum(default)]
    Invalid = u64::MAX,
}

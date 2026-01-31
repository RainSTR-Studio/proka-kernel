//! Model-Specific Registers (MSRs) definitions

/// APIC Location and Status (R/W) See Table 35-2. See Section 10.4.4, Local APIC  Status and Location.
pub const IA32_APIC_BASE: u32 = 0x1b;

/// If ( CPUID.01H:ECX.[bit 21]  = 1 )
pub const IA32_X2APIC_EOI: u32 = 0x80b;

/// x2APIC Spurious Interrupt Vector register (R/W)
pub const IA32_X2APIC_SIVR: u32 = 0x80f;

/// x2APIC ID register (R/O) See x2APIC Specification.
pub const IA32_X2APIC_APICID: u32 = 0x802;

/// If ( CPUID.01H:ECX.[bit 21]  = 1 )
pub const IA32_X2APIC_VERSION: u32 = 0x803;

/// x2APIC Task Priority register (R/W)
pub const IA32_X2APIC_TPR: u32 = 0x808;

/// x2APIC Processor Priority register (R/O)
pub const IA32_X2APIC_PPR: u32 = 0x80a;

/// x2APIC Logical Destination register (R/O)
pub const IA32_X2APIC_LDR: u32 = 0x80d;

/// x2APIC In-Service register bits [31:0] (R/O)
pub const IA32_X2APIC_ISR0: u32 = 0x810;

/// x2APIC In-Service register bits [63:32] (R/O)
pub const IA32_X2APIC_ISR1: u32 = 0x811;

/// x2APIC In-Service register bits [95:64] (R/O)
pub const IA32_X2APIC_ISR2: u32 = 0x812;

/// x2APIC In-Service register bits [127:96] (R/O)
pub const IA32_X2APIC_ISR3: u32 = 0x813;

/// x2APIC In-Service register bits [159:128] (R/O)
pub const IA32_X2APIC_ISR4: u32 = 0x814;

/// x2APIC In-Service register bits [191:160] (R/O)
pub const IA32_X2APIC_ISR5: u32 = 0x815;

/// x2APIC In-Service register bits [223:192] (R/O)
pub const IA32_X2APIC_ISR6: u32 = 0x816;

/// x2APIC In-Service register bits [255:224] (R/O)
pub const IA32_X2APIC_ISR7: u32 = 0x817;

/// x2APIC Trigger Mode register bits [31:0] (R/O)
pub const IA32_X2APIC_TMR0: u32 = 0x818;

/// x2APIC Trigger Mode register bits [63:32] (R/O)
pub const IA32_X2APIC_TMR1: u32 = 0x819;

/// x2APIC Trigger Mode register bits [95:64] (R/O)
pub const IA32_X2APIC_TMR2: u32 = 0x81a;

/// x2APIC Trigger Mode register bits [127:96] (R/O)
pub const IA32_X2APIC_TMR3: u32 = 0x81b;

/// x2APIC Trigger Mode register bits [159:128] (R/O)
pub const IA32_X2APIC_TMR4: u32 = 0x81c;

/// x2APIC Trigger Mode register bits [191:160] (R/O)
pub const IA32_X2APIC_TMR5: u32 = 0x81d;

/// x2APIC Trigger Mode register bits [223:192] (R/O)
pub const IA32_X2APIC_TMR6: u32 = 0x81e;

/// x2APIC Trigger Mode register bits [255:224] (R/O)
pub const IA32_X2APIC_TMR7: u32 = 0x81f;

/// x2APIC Interrupt Request register bits [31:0] (R/O)
pub const IA32_X2APIC_IRR0: u32 = 0x820;

/// x2APIC Interrupt Request register bits [63:32] (R/O)
pub const IA32_X2APIC_IRR1: u32 = 0x821;

/// x2APIC Interrupt Request register bits [95:64] (R/O)
pub const IA32_X2APIC_IRR2: u32 = 0x822;

/// x2APIC Interrupt Request register bits [127:96] (R/O)
pub const IA32_X2APIC_IRR3: u32 = 0x823;

/// x2APIC Interrupt Request register bits [159:128] (R/O)
pub const IA32_X2APIC_IRR4: u32 = 0x824;

/// x2APIC Interrupt Request register bits [191:160] (R/O)
pub const IA32_X2APIC_IRR5: u32 = 0x825;

/// x2APIC Interrupt Request register bits [223:192] (R/O)
pub const IA32_X2APIC_IRR6: u32 = 0x826;

/// x2APIC Interrupt Request register bits [255:224] (R/O)
pub const IA32_X2APIC_IRR7: u32 = 0x827;

/// If ( CPUID.01H:ECX.[bit 21]  = 1 )
pub const IA32_X2APIC_ESR: u32 = 0x828;

/// x2APIC LVT Corrected Machine Check Interrupt register (R/W)
pub const IA32_X2APIC_LVT_CMCI: u32 = 0x82f;

/// x2APIC Interrupt Command register (R/W)
pub const IA32_X2APIC_ICR: u32 = 0x830;

/// x2APIC LVT Timer Interrupt register (R/W)
pub const IA32_X2APIC_LVT_TIMER: u32 = 0x832;

/// x2APIC LVT Thermal Sensor Interrupt register (R/W)
pub const IA32_X2APIC_LVT_THERMAL: u32 = 0x833;

/// x2APIC LVT Performance Monitor register (R/W)
pub const IA32_X2APIC_LVT_PMI: u32 = 0x834;

/// If ( CPUID.01H:ECX.[bit 21]  = 1 )
pub const IA32_X2APIC_LVT_LINT0: u32 = 0x835;

/// If ( CPUID.01H:ECX.[bit 21]  = 1 )
pub const IA32_X2APIC_LVT_LINT1: u32 = 0x836;

/// If ( CPUID.01H:ECX.[bit 21]  = 1 )
pub const IA32_X2APIC_LVT_ERROR: u32 = 0x837;

/// x2APIC Initial Count register (R/W)
pub const IA32_X2APIC_INIT_COUNT: u32 = 0x838;

/// x2APIC Current Count register (R/O)
pub const IA32_X2APIC_CUR_COUNT: u32 = 0x839;

/// x2APIC Divide Configuration register (R/W)
pub const IA32_X2APIC_DIV_CONF: u32 = 0x83e;

/// If ( CPUID.01H:ECX.[bit 21]  = 1 )
pub const IA32_X2APIC_SELF_IPI: u32 = 0x83f;

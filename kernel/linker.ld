/* Output as the ELF file (will be raw binary later) */
OUTPUT_FORMAT(elf64-x86-64)

/* The kernel entry point */
ENTRY(kernel_main)

/* Sections */
SECTIONS {
    /* Load at half-high address (proka-bootloader) */
    . = 0xffff800000000000;

    /* Texts */
    .text : {
	*(.header*)
	*(.main*)
        *(.text*)
    }

    /* Move to the next memory page */
    . = ALIGN(CONSTANT(MAXPAGESIZE));

    .data : {
        *(.data*)
    }

    /* Move to the next memory page */
    . = ALIGN(CONSTANT(MAXPAGESIZE));

    .rodata : {
        *(.rodata*)
    }

    /* Move to the next memory page */
    . = ALIGN(CONSTANT(MAXPAGESIZE));

    .bss : {
        *(.bss*)
    }

    /DISCARD/ : {
        *(.comment*)
        *(.note*)
        *(.eh_frame*)
    }
}

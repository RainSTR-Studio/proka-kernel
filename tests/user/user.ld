/*
 * Linker script for user-mode test program
 */

ENTRY(_start)

SECTIONS
{
    /* User space starts at 0x400000 (typical for Linux) */
    . = 0x400000;

    .text : {
        *(.text)
        *(.text.*)
    }

    .rodata : {
        *(.rodata)
        *(.rodata.*)
    }

    .data : {
        *(.data)
        *(.data.*)
    }

    .bss : {
        *(.bss)
        *(.bss.*)
        *(COMMON)
    }

    /DISCARD/ : {
        *(.comment)
        *(.note)
        *(.eh_frame)
    }
}

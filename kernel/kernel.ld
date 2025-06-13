SECTIONS {
    . = 0x2000000;

    .text : ALIGN(4096) {
        __text = .;
        *(.text .text.*);
        __text_end = .;
    }

    .rodata : ALIGN(4096) {
        __rodata = .;
        *(.rodata .rodata.*);
        __rodata_end = .;
    }

    .data : ALIGN(4096) {
        __data = .;
        *(.data .data.*);
        __data_end = .;
    }

    .bss : ALIGN(4096) {
        __bss = .;
        *(.bss .bss.* .sbss .sbss.*);
        __bss_end = .;
    }
}

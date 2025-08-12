KERNEL_VADDR_BASE = 0xFFFFFFFF80000000;
KERNEL_VADDR_TEXT = 0xFFFFFFFF80100000;

SECTIONS {
    . = 0xFFFFFFFF80100000;

    .text ALIGN(4K) : AT (ADDR(.text) - KERNEL_VADDR_BASE) {
        __text = .;
        *(.text .text.*);
        __text_end = .;
    }

    .rodata ALIGN(4k) : AT (ADDR(.rodata) - KERNEL_VADDR_BASE) {
        __rodata = .;
        *(.rodata .rodata.*);
        __rodata_end = .;
    }

    .data ALIGN(4k) : AT (ADDR(.data) - KERNEL_VADDR_BASE) {
        __data = .;
        *(.data .data.*);
        __data_end = .;
    }

    .bss ALIGN(4k) : AT (ADDR(.bss) - KERNEL_VADDR_BASE) {
        __bss = .;
        *(.bss .bss.* .sbss .sbss.*);
        __bss_end = .;
    }
}

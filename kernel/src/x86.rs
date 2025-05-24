use core::arch::asm;

pub fn write_io(port: u16, value: u8) {
    unsafe {
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
        );
    }
}

pub fn read_io(port: u16) -> u8 {
    let mut value: u8;
    unsafe {
        asm!(
            "in al, dx",
            in("dx") port,
            out("al") value,
        );
    }
    value
}

#[allow(dead_code)]
pub fn disable_interrupts() {
    unsafe {
        asm!(
            "cli",
            options(nostack),
        );
    }
}

pub fn enable_interrupts() {
    unsafe {
        asm!(
            "sti",
            options(nostack),
        );
    }
}

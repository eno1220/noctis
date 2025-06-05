use crate::x86::write_io;
use core::arch::asm;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Sucess = 1,
    Fail = 2,
}

#[allow(dead_code)]
pub fn exit_qemu(exit_code: QemuExitCode) -> ! {
    write_io(0xf4, exit_code as u8);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

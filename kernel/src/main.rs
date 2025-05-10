#![no_std]
#![no_main]

mod log;
mod uart;
mod x86;

use crate::uart::Uart;
use core::arch::asm;
use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub extern "C" fn kernel_entry(stack_base: u64) -> ! {
    loop {
        unsafe {
            asm!(
                "mov rsp, {0}",
                "call kernel_main",
                in(reg) stack_base,
            );
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn kernel_main() -> ! {
    Uart::default().init();
    info!("Kernel started!");

    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    error!("Kernel panic!");
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

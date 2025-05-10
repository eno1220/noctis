#![no_std]
#![no_main]
#![feature(naked_functions_rustic_abi)]

extern crate alloc;

mod allocator;
mod gdt;
mod log;
mod spin;
mod uart;
mod x86;

use core::arch::asm;
use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub extern "C" fn kernel_entry(stack_base: u64, heap_base: u64, heap_size: u64) -> ! {
    loop {
        unsafe {
            asm!(
                "mov rsp, {0}",
                "mov rdi, {1}",
                "mov rsi, {2}",
                "call kernel_main",
                in(reg) stack_base,
                in(reg) heap_base,
                in(reg) heap_size,
            );
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn kernel_main(heap_base: u64, heap_size: u64) -> ! {
    uart::Uart::default().init();
    info!("Kernel started!");

    allocator::init_allocator(heap_base as usize, heap_size as usize);
    info!("Allocator initialized!");

    let _gdt = gdt::init_gdt();

    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[panic_handler]
fn panic(panic_info: &PanicInfo) -> ! {
    error!("!!!!! Kernel panic !!!!!");
    error!("Panic info: {:?}", panic_info);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

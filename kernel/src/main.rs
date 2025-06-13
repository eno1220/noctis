#![no_main]
#![no_std]
#![feature(naked_functions_rustic_abi)]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

mod allocator;
mod gdt;
mod idt;
mod log;
mod paging;
mod qemu;
mod spin;
mod timer;
mod uart;
mod x86;

#[cfg(test)]
mod test;

use core::arch::asm;
#[allow(unused_imports)]
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

    print!(
        r#"
                     _    _
 _ __    ___    ___ | |_ (_) ___
| '_ \  / _ \  / __|| __|| |/ __|
| | | || (_) || (__ | |_ | |\__ \
|_| |_| \___/  \___| \__||_||___/


"#
    );
    info!("Kernel started!");

    allocator::init_allocator(heap_base as usize, heap_size as usize);
    info!("Allocator initialized!");

    let _ = paging::init_paging();
    info!("Paging initialized!");

    let _gdt = gdt::init_gdt();
    let _idt = idt::init_idt();
    info!("GDT and IDT initialized!");

    timer::init_timer();
    info!("Timer initialized!");

    #[cfg(test)]
    test_main();

    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

// ref: https://github.com/redox-os/kernel/blob/master/src/main.rs
macro_rules! symbol_offsets(
    ($($name:ident), *) => {
        $(
            #[inline]
            pub fn $name() -> usize {
                unsafe extern "C" {
                    static $name: u8;
                }
                unsafe { &$name as *const u8 as usize }
            }
        )*
    };
);

mod symbol_offsets {
    symbol_offsets!(
        __text,
        __text_end,
        __rodata,
        __rodata_end,
        __data,
        __data_end,
        __bss,
        __bss_end
    );
}

#[cfg(not(test))]
#[panic_handler]
fn panic(panic_info: &PanicInfo) -> ! {
    error!("!!!!! Kernel panic !!!!!");
    error!("Panic info: {:?}", panic_info);
    loop {
        unsafe { asm!("hlt") }
    }
}

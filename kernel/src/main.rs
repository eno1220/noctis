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
mod memlayout;
mod memory;
mod paging;
mod qemu;
mod spin;
mod task;
mod timer;
mod uart;
mod wasm;
mod x86;

#[cfg(test)]
mod test;

use core::arch::asm;
#[allow(unused_imports)]
use core::panic::PanicInfo;

// FIXME: modify to pass BOOT_INFO as a unified structure to the kernel
#[unsafe(no_mangle)]
pub extern "C" fn kernel_entry(stack_base: u64, heap_base: u64, heap_size: u64,  memory_region: &memory::MemoryRegionArray) -> ! {
    loop {
        unsafe {
            asm!(
                "mov rsp, {0}",
                "mov rdi, {1}",
                "mov rsi, {2}",
				"mov rdx, {3}",
                "call kernel_main",
                in(reg) stack_base,
                in(reg) heap_base,
                in(reg) heap_size,
				in(reg) memory_region,
            );
        }
    }
}

fn task_a() {
    loop {
        print!("a");
        unsafe { asm!("hlt") }
    }
}

fn task_b() {
    loop {
        print!("b");
        unsafe { asm!("hlt") }
    }
}

#[unsafe(no_mangle)]
extern "C" fn kernel_main(heap_base: u64, heap_size: u64, _: &memory::MemoryRegionArray) -> ! {
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

    let pt = paging::init_paging();
    info!("Paging initialized!");

    let _gdt = gdt::init_gdt();
    let _idt = idt::init_idt();
    info!("GDT and IDT initialized!");

    timer::init_timer();
    info!("Timer initialized!");

    x86::disable_interrupts();
    task::init(pt);
    task::spawn(task_a);
    task::spawn(task_b);
    task::spawn(wasm::wasm_entry);
    x86::enable_interrupts();

    #[cfg(test)]
    test_main();

    loop {
        unsafe { asm!("hlt") }
    }
}

// ref: https://github.com/redox-os/kernel/blob/master/src/main.rs
macro_rules! symbol_offsets(
    ($($name:ident), *) => {
        $(
            #[inline]
            pub fn $name() -> crate::memlayout::VirtAddr {
                unsafe extern "C" {
                    static $name: u8;
                }
                unsafe { crate::memlayout::VirtAddr::new(&$name as *const u8 as usize) }
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
    unsafe { asm!("cli") }
    error!("!!!!! Kernel panic !!!!!");
    error!("Panic info: {:?}", panic_info);
    loop {
        unsafe { asm!("hlt") }
    }
}

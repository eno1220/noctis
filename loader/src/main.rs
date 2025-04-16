#![feature(uefi_std)]

use core::arch::asm;
use r_efi::efi::{self, MemoryDescriptor};
use std::os::uefi;

#[allow(dead_code)]
struct MemoryMap {
    map_size: usize,
    buffer: *mut u8,
    map_key: usize,
    desc_size: usize,
    desc_ver: u32,
}

fn get_memory_map(bt: *const efi::BootServices) -> Result<MemoryMap, efi::Status> {
    let mut map_size = 0;
    let mut map_key = 0;
    let mut desc_size = 0;
    let mut desc_ver = 0;

    let status = unsafe {
        ((*bt).get_memory_map)(
            &mut map_size,
            core::ptr::null_mut(),
            &mut map_key,
            &mut desc_size,
            &mut desc_ver,
        )
    };
    if status != efi::Status::BUFFER_TOO_SMALL {
        return Err(status);
    }

    map_size += desc_size * 4;

    let mut buffer: *mut u8 = core::ptr::null_mut();
    let status = unsafe {
        ((*bt).allocate_pool)(
            efi::LOADER_DATA,
            map_size,
            &mut buffer as *mut *mut u8 as *mut *mut core::ffi::c_void,
        )
    };
    if status != efi::Status::SUCCESS {
        return Err(status);
    }

    let status = unsafe {
        ((*bt).get_memory_map)(
            &mut map_size,
            buffer as *mut efi::MemoryDescriptor,
            &mut map_key,
            &mut desc_size,
            &mut desc_ver,
        )
    };
    if status != efi::Status::SUCCESS {
        return Err(status);
    }

    Ok(MemoryMap {
        map_size,
        buffer,
        map_key,
        desc_size,
        desc_ver,
    })
}

fn print_memory_map(memory_map: &MemoryMap) {
    let num_of_entries = memory_map.map_size / memory_map.desc_size;
    for i in 0..num_of_entries {
        let desc = unsafe {
            let ptr = memory_map.buffer.add(i * memory_map.desc_size) as *const MemoryDescriptor;
            &*ptr
        };
        println!(
            "[{:#03}] start: {:#012x}, len: {:#06} KiB, type: {:?}",
            i,
            desc.physical_start,
            desc.number_of_pages * 4,
            desc.r#type
        );
    }
}

fn main() {
    //let st = uefi::env::system_table().as_ptr() as *const efi::SystemTable;
    let bt = uefi::env::boot_services().unwrap().as_ptr() as *const efi::BootServices;

    println!("Hello, world!");

    let memory_map = get_memory_map(bt).unwrap();
    print_memory_map(&memory_map);

    loop {
        unsafe { asm!("hlt") };
    }
}

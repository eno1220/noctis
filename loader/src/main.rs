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

impl MemoryMap {
    fn iter(&self) -> MemoryMapIterator {
        MemoryMapIterator {
            map: self,
            offset: 0,
        }
    }
}

struct MemoryMapIterator<'a> {
    map: &'a MemoryMap,
    offset: usize,
}

impl <'a> Iterator for MemoryMapIterator<'a> {
    type Item = &'a MemoryDescriptor;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.map.map_size {
            return None;
        }
        let desc = unsafe {
            &*(self.map.buffer.add(self.offset) as *const MemoryDescriptor)
        };
        self.offset += self.map.desc_size;
        Some(desc)
    }
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

fn main() {
    //let st = uefi::env::system_table().as_ptr() as *const efi::SystemTable;
    let bt = uefi::env::boot_services().unwrap().as_ptr() as *const efi::BootServices;

    println!("Hello, world!");

    let memory_map = match get_memory_map(bt) {
        Ok(map) => map,
        Err(status) => {
            println!("Failed to get memory map: {:?}", status);
            return;
        }
    };
    for desc in memory_map.iter() {
        println!(
            "Physical Start: {:#018x}, Number of Pages: {:#07x}, Type: {:?}",
            desc.physical_start, desc.number_of_pages, desc.r#type
        );
    }

    loop {
        unsafe { asm!("hlt") };
    }
}

#![feature(uefi_std)]

use core::arch::asm;
use r_efi::efi::{self, MemoryDescriptor};
use std::{
    ffi::OsStr,
    os::uefi::{self, ffi::OsStrExt},
};

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

impl<'a> Iterator for MemoryMapIterator<'a> {
    type Item = &'a MemoryDescriptor;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.map.map_size {
            return None;
        }
        let desc = unsafe { &*(self.map.buffer.add(self.offset) as *const MemoryDescriptor) };
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

#[allow(const_item_mutation)]
fn get_root_dir(
    bt: *const efi::BootServices,
) -> Result<*mut efi::protocols::file::Protocol, efi::Status> {
    let mut simple_file_system: *mut efi::protocols::simple_file_system::Protocol =
        core::ptr::null_mut();
    let status = unsafe {
        ((*bt).locate_protocol)(
            &mut efi::protocols::simple_file_system::PROTOCOL_GUID as *mut efi::Guid,
            core::ptr::null_mut(),
            &mut simple_file_system as *mut *mut efi::protocols::simple_file_system::Protocol
                as *mut *mut core::ffi::c_void,
        )
    };
    if status != efi::Status::SUCCESS {
        return Err(status);
    }

    let mut root_dir: *mut efi::protocols::file::Protocol = core::ptr::null_mut();
    let status = unsafe { ((*simple_file_system).open_volume)(simple_file_system, &mut root_dir) };
    if status != efi::Status::SUCCESS {
        return Err(status);
    }
    Ok(root_dir)
}

#[allow(const_item_mutation)]
fn main() {
    //let st = uefi::env::system_table().as_ptr() as *const efi::SystemTable;
    let bt = uefi::env::boot_services().unwrap().as_ptr() as *const efi::BootServices;

    println!("Hello, world!");

    let memory_map = match get_memory_map(bt) {
        Ok(map) => map,
        Err(status) => {
            println!("Failed to get memory map: {status:?}");
            return;
        }
    };
    for desc in memory_map.iter() {
        println!(
            "Physical Start: {:#018x}, Number of Pages: {:#07x}, Type: {:?}",
            desc.physical_start, desc.number_of_pages, desc.r#type
        );
    }

    let root_dir = match get_root_dir(bt) {
        Ok(dir) => dir,
        Err(status) => {
            println!("Failed to get root directory: {status:?}");
            return;
        }
    };

    let mut kernel_file: *mut efi::protocols::file::Protocol = core::ptr::null_mut();
    let mut kernel_file_name: Vec<u16> = OsStr::new("kernel.elf")
        .encode_wide()
        .chain(Some(0)) // Null-terminate the string
        .collect();
    let status = unsafe {
        ((*root_dir).open)(
            root_dir,
            &mut kernel_file as *mut *mut efi::protocols::file::Protocol,
            kernel_file_name.as_mut_ptr() as *mut efi::Char16,
            efi::protocols::file::MODE_READ,
            0,
        )
    };
    if status != efi::Status::SUCCESS {
        println!("Failed to open kernel file: {status:?}");
        return;
    }

    let file_info: *mut efi::protocols::file::Info =
        [0u8; core::mem::size_of::<efi::protocols::file::Info>() + 1024].as_mut_ptr()
            as *mut efi::protocols::file::Info;
    let mut file_info_size = core::mem::size_of::<efi::protocols::file::Info>() + 1024;
    let status = unsafe {
        ((*kernel_file).get_info)(
            kernel_file,
            &mut efi::protocols::file::INFO_ID,
            &mut file_info_size,
            file_info as *mut core::ffi::c_void,
        )
    };
    if status != efi::Status::SUCCESS {
        println!("Failed to get file info: {status:?}");
        return;
    }
    println!("File Size: {}", unsafe { (*file_info).file_size });

    loop {
        unsafe { asm!("hlt") };
    }
}

#![feature(uefi_std)]

mod memory;

use core::arch::asm;
use elf::{ElfBytes, endian::AnyEndian};
use r_efi::{efi, system};
use std::{
    ffi::OsStr,
    os::uefi::{self, ffi::OsStrExt},
};

const KERNEL_STACK_SIZE: u64 = 0x4000;
const KERNEL_HEAP_SIZE: u64 = 0x1000000;

fn open_root_dir() -> *mut efi::protocols::file::Protocol {
    let bt = uefi::env::boot_services().unwrap().as_ptr() as *const efi::BootServices;

    let mut simple_file_system: *mut efi::protocols::simple_file_system::Protocol =
        core::ptr::null_mut();
    status_to_result(unsafe {
        #[allow(const_item_mutation)]
        ((*bt).locate_protocol)(
            &mut efi::protocols::simple_file_system::PROTOCOL_GUID as *mut efi::Guid,
            core::ptr::null_mut(),
            &mut simple_file_system as *mut *mut efi::protocols::simple_file_system::Protocol
                as *mut *mut core::ffi::c_void,
        )
    })
    .expect("Failed to locate Simple File System Protocol");

    let mut root_dir: *mut efi::protocols::file::Protocol = core::ptr::null_mut();
    status_to_result(unsafe {
        ((*simple_file_system).open_volume)(
            simple_file_system,
            &mut root_dir as *mut *mut efi::protocols::file::Protocol,
        )
    })
    .expect("Failed to open volume");

    root_dir
}

fn open_kernel_file(
    root_dir: *mut efi::protocols::file::Protocol,
) -> (*mut efi::protocols::file::Protocol, usize) {
    let mut kernel_file: *mut efi::protocols::file::Protocol = core::ptr::null_mut();
    let mut kernel_file_name: Vec<u16> = OsStr::new("kernel.elf")
        .encode_wide()
        .chain(Some(0))
        .collect();
    status_to_result(unsafe {
        ((*root_dir).open)(
            root_dir,
            &mut kernel_file as *mut *mut efi::protocols::file::Protocol,
            kernel_file_name.as_mut_ptr() as *mut efi::Char16,
            efi::protocols::file::MODE_READ,
            0,
        )
    })
    .expect("Failed to open kernel file");

    let file_info: *mut efi::protocols::file::Info =
        [0u8; core::mem::size_of::<efi::protocols::file::Info>() + 1024].as_mut_ptr()
            as *mut efi::protocols::file::Info;
    let mut file_info_size = core::mem::size_of::<efi::protocols::file::Info>() + 1024;
    status_to_result(unsafe {
        #[allow(const_item_mutation)]
        ((*kernel_file).get_info)(
            kernel_file,
            &mut efi::protocols::file::INFO_ID,
            &mut file_info_size,
            file_info as *mut core::ffi::c_void,
        )
    })
    .expect("Failed to get file info");
    let kernel_file_size = unsafe { (*file_info).file_size };
    println!("Kernel File Size: {kernel_file_size:#018x}");

    (kernel_file, kernel_file_size as usize)
}

fn read_kernel_file(
    kernel_file: *mut efi::protocols::file::Protocol,
    mut kernel_file_size: usize,
    kernel_ref: &mut Vec<u8>,
) -> usize {
    let kernel_ref = kernel_ref.as_mut_ptr();
    status_to_result(unsafe {
        ((*kernel_file).read)(
            kernel_file,
            &mut kernel_file_size as *mut usize,
            kernel_ref as *mut core::ffi::c_void,
        )
    })
    .expect("Failed to read kernel file");

    kernel_file_size
}

fn load_to_memory(kernel_ref: Vec<u8>, kernel_file_size: usize) -> usize {
    let bt = uefi::env::boot_services().unwrap().as_ptr() as *const efi::BootServices;

    let kernel_elf = ElfBytes::<AnyEndian>::minimal_parse(unsafe {
        core::slice::from_raw_parts(kernel_ref.as_ptr(), kernel_file_size)
    })
    .expect("Failed to parse ELF file");

    let (mut kernel_base, kernel_end) = get_kernel_size(&kernel_elf);
    let kernel_entry = kernel_elf.ehdr.e_entry as usize;
    println!("Kernel Entry Point: {kernel_entry:#018x}");

    #[allow(clippy::manual_div_ceil)]
    status_to_result(unsafe {
        ((*bt).allocate_pages)(
            system::ALLOCATE_ADDRESS,
            efi::LOADER_DATA,
            (((kernel_end - kernel_base) + 0xFFF) / 0x1000) as usize,
            &mut kernel_base as *mut u64,
        )
    })
    .expect("Failed to allocate memory for kernel");

    for ph in kernel_elf
        .segments()
        .unwrap()
        .into_iter()
        .filter(|ph| ph.p_type == elf::abi::PT_LOAD)
    {
        let segment_src = unsafe { kernel_ref.as_ptr().add(ph.p_offset as usize) };
        let segment_size = ph.p_filesz as usize;
        let segment_dst = (ph.p_vaddr) as *mut u8;
        unsafe {
            core::ptr::copy_nonoverlapping(segment_src, segment_dst, segment_size);
            core::ptr::write_bytes(
                segment_dst.add(segment_size),
                0,
                ph.p_memsz as usize - segment_size,
            );
        }
    }

    kernel_entry
}

fn get_kernel_size(file: &elf::ElfBytes<AnyEndian>) -> (u64, u64) {
    let mut min_addr = u64::MAX;
    let mut max_addr = 0;
    for ph in file
        .segments()
        .unwrap()
        .into_iter()
        .filter(|ph| ph.p_type == elf::abi::PT_LOAD)
    {
        min_addr = min_addr.min(ph.p_vaddr);
        max_addr = max_addr.max(ph.p_vaddr + ph.p_memsz);
    }
    (min_addr, max_addr)
}

fn allocate_memory(size: u64) -> u64 {
    let bt = uefi::env::boot_services().unwrap().as_ptr() as *const efi::BootServices;
    let mut addr: u64 = 0;

    status_to_result(unsafe {
        ((*bt).allocate_pages)(
            system::ALLOCATE_ANY_PAGES,
            efi::LOADER_DATA,
            ((size + 0xFFF) / 0x1000) as usize,
            &mut addr as *mut u64,
        )
    })
    .expect("Failed to allocate memory");

    addr
}

pub fn status_to_result(status: efi::Status) -> Result<(), efi::Status> {
    match status {
        efi::Status::SUCCESS => Ok(()),
        _ => Err(status),
    }
}

#[allow(const_item_mutation)]
fn main() {
    let handle = uefi::env::image_handle().as_ptr() as *mut efi::Handle;
    let bt = uefi::env::boot_services().unwrap().as_ptr() as *const efi::BootServices;

    println!("Hello, world!");

    /*let memory_map = memory::MemoryMap::new();
    for desc in memory_map.iter() {
        println!(
            "Physical Start: {:#018x}, Number of Pages: {:#07x}, Type: {:?}",
            desc.physical_start, desc.number_of_pages, desc.r#type
        );
    }*/

    let root_dir = open_root_dir();
    let (kernel_file, mut kernel_file_size) = open_kernel_file(root_dir);
    let mut kernel_ref = vec![0u8; kernel_file_size + 1024];
    kernel_file_size = read_kernel_file(kernel_file, kernel_file_size + 1024, &mut kernel_ref);
    let kernel_entry = load_to_memory(kernel_ref, kernel_file_size);

    let stack_base = allocate_memory(KERNEL_STACK_SIZE) + KERNEL_STACK_SIZE;
    let heap_base = allocate_memory(KERNEL_HEAP_SIZE);
    let heap_size: u64 = KERNEL_HEAP_SIZE;

    let memory_map = memory::MemoryMap::new();

    status_to_result(unsafe {
        ((*bt).exit_boot_services)(handle as *mut core::ffi::c_void, memory_map.get_map_key())
    })
    .expect("Failed to exit boot services");

    unsafe {
        let kernel_entry: extern "sysv64" fn(stack_base: u64, heap_base: u64, heap_size: u64) -> ! =
            core::mem::transmute(kernel_entry);
        kernel_entry(stack_base as u64, heap_base as u64, heap_size as u64);
    }

    #[allow(unreachable_code)]
    loop {
        unsafe { asm!("hlt") };
    }
}

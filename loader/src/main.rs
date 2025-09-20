#![feature(uefi_std)]

mod memory;
mod paging;

use core::arch::asm;
use elf::{ElfBytes, endian::AnyEndian};
use r_efi::{efi, system};
use std::{
    ffi::OsStr,
    os::uefi::{self, ffi::OsStrExt},
};

use crate::{
    memory::{MemoryRegion, MemoryRegionArray, PAGE_SIZE},
    paging::{MSize, PhysAddr, VirtAddr, KERNEL_DIRECT_START},
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

fn get_image_base() -> usize {
    let bt = uefi::env::boot_services().unwrap().as_ptr() as *const efi::BootServices;

    let mut loaded_image_protocol: *mut efi::protocols::loaded_image::Protocol =
        core::ptr::null_mut();
    status_to_result(unsafe {
        #[allow(const_item_mutation)]
        ((*bt).locate_protocol)(
            &mut efi::protocols::loaded_image::PROTOCOL_GUID as *mut efi::Guid,
            core::ptr::null_mut(),
            &mut loaded_image_protocol as *mut *mut efi::protocols::loaded_image::Protocol
                as *mut *mut core::ffi::c_void,
        )
    })
    .expect("Failed to locate Loaded Image Protocol");

    unsafe { (*loaded_image_protocol).image_base as usize }
}

fn load_to_memory(kernel_ref: Vec<u8>, kernel_file_size: usize) -> usize {
    let bt = uefi::env::boot_services().unwrap().as_ptr() as *const efi::BootServices;

    let kernel_elf = ElfBytes::<AnyEndian>::minimal_parse(unsafe {
        core::slice::from_raw_parts(kernel_ref.as_ptr(), kernel_file_size)
    })
    .expect("Failed to parse ELF file");

    let (kernel_start_virt, kernel_start_phys, kernel_end_virt) = get_kernel_size(&kernel_elf);
    let kernel_entry = kernel_elf.ehdr.e_entry as usize;
    println!("Kernel Entry Point: {kernel_entry:#018x}");

    paging::init_early_paging(
        PhysAddr::new(kernel_start_phys),
        VirtAddr::new(kernel_start_virt),
        MSize::new(kernel_end_virt - kernel_start_virt),
    );

    #[allow(clippy::manual_div_ceil)]
    status_to_result(unsafe {
        ((*bt).allocate_pages)(
            system::ALLOCATE_ADDRESS,
            efi::LOADER_DATA,
            (((kernel_end_virt - kernel_start_virt) + 0xFFF) / 0x1000) as usize,
            &mut (kernel_start_phys as u64) as *mut u64,
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
        let segment_dst = (ph.p_paddr) as *mut u8;
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

fn get_kernel_size(file: &elf::ElfBytes<AnyEndian>) -> (usize, usize, usize) {
    let mut kernel_start_virt = usize::MAX;
    let mut kernel_start_phys = usize::MAX;
    let mut kernel_end_virt = 0;
    for ph in file
        .segments()
        .unwrap()
        .into_iter()
        .filter(|ph| ph.p_type == elf::abi::PT_LOAD)
    {
        kernel_start_virt = kernel_start_virt.min(ph.p_vaddr as usize);
        kernel_start_phys = kernel_start_phys.min(ph.p_paddr as usize);
        kernel_end_virt = kernel_end_virt.max((ph.p_vaddr + ph.p_memsz) as usize);
    }
    (kernel_start_virt, kernel_start_phys, kernel_end_virt)
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

    let memory_map = memory::MemoryMap::new();
    /*for desc in memory_map.iter() {
        println!(
            "Physical Start: {:#018x}, Number of Pages: {:#07x}, Type: {:?}",
            desc.physical_start, desc.number_of_pages, desc.r#type
        );
    }*/

    println!("Image Base: {:#x}", get_image_base());

    let root_dir = open_root_dir();
    let (kernel_file, mut kernel_file_size) = open_kernel_file(root_dir);
    let mut kernel_ref = vec![0u8; kernel_file_size + 1024];
    kernel_file_size = read_kernel_file(kernel_file, kernel_file_size + 1024, &mut kernel_ref);
    let kernel_entry = load_to_memory(kernel_ref, kernel_file_size);

    let stack_base =
        allocate_memory(KERNEL_STACK_SIZE) + KERNEL_STACK_SIZE + (KERNEL_DIRECT_START as u64);
    let heap_base = allocate_memory(KERNEL_HEAP_SIZE) + (KERNEL_DIRECT_START as u64);
    let heap_size: u64 = KERNEL_HEAP_SIZE;

	// FIXME:
	// While strictly speaking one should use the memory map obtained immediately before calling the kernel,
	// this implementation avoids changing the memory map revision by using heap memory within the processing.
    let mut memory_regions = MemoryRegionArray::new();
    for desc in memory_map.iter() {
        let region = match desc.r#type {
			// While BOOT_SERVICES_DATA could also be used here,
			// it includes page tables and thus isn't employed for this purpose.
            efi::BOOT_SERVICES_CODE | efi::CONVENTIONAL_MEMORY => MemoryRegion::new(
                desc.physical_start as usize,
                desc.number_of_pages as usize * PAGE_SIZE,
                memory::MemoryRegionType::Usable,
            ),
            _ => MemoryRegion::new(
                desc.physical_start as usize,
                desc.number_of_pages as usize * PAGE_SIZE,
                memory::MemoryRegionType::Reserved,
            ),
        };
        memory_regions.push(region);
    }

    let memory_map = memory::MemoryMap::new();

    status_to_result(unsafe {
        ((*bt).exit_boot_services)(handle as *mut core::ffi::c_void, memory_map.get_map_key())
    })
    .expect("Failed to exit boot services");

    unsafe {
        let kernel_entry: extern "sysv64" fn(stack_base: u64, heap_base: u64, heap_size: u64, memory_region: &MemoryRegionArray) -> ! =
            core::mem::transmute(kernel_entry);
        kernel_entry(stack_base as u64, heap_base as u64, heap_size as u64, &memory_regions);
    }

    #[allow(unreachable_code)]
    loop {
        unsafe { asm!("hlt") };
    }
}

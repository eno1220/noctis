use super::status_to_result;
use r_efi::efi::{self, MemoryDescriptor};
use std::os::uefi;

#[derive(Clone, Copy, Debug)]
pub enum MemoryRegionType {
    Reserved,
    Usable,
}

#[derive(Clone, Copy, Debug)]
pub struct MemoryRegion {
    base: usize,
    len: usize,
    typ: MemoryRegionType,
}

#[allow(dead_code)]
impl MemoryRegion {
    pub const fn new(base: usize, len: usize, typ: MemoryRegionType) -> Self {
        MemoryRegion { base, len, typ }
    }

    pub const fn zeroed() -> Self {
        MemoryRegion {
            base: 0,
            len: 0,
            typ: MemoryRegionType::Reserved,
        }
    }

    pub fn base(&self) -> usize {
        self.base
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn typ(&self) -> MemoryRegionType {
        self.typ
    }
}

pub const PAGE_SIZE: usize = 4096;
const MAX_MEMORY_REGION_LEN: usize = 128;

pub struct MemoryRegionArray {
    regions: [MemoryRegion; MAX_MEMORY_REGION_LEN],
    count: usize,
}

impl MemoryRegionArray {
    pub const fn new() -> Self {
        Self {
            regions: [MemoryRegion::zeroed(); MAX_MEMORY_REGION_LEN],
            count: 0,
        }
    }

    pub fn push(&mut self, region: MemoryRegion) {
        // FIXME: Set the return value to result,
        // and return an Error if the array is full.
        assert!(self.count < MAX_MEMORY_REGION_LEN);
        self.regions[self.count] = region;
        self.count += 1;
    }
}

#[allow(dead_code)]
pub struct MemoryMap {
    map: Vec<u8>,
    map_size: usize,
    map_key: usize,
    desc_size: usize,
    desc_ver: u32,
}

impl MemoryMap {
    pub fn new() -> Self {
        let bt = uefi::env::boot_services().unwrap().as_ptr() as *const efi::BootServices;

        let mut map_key = 0;
        let mut desc_size = 0;
        let mut desc_ver = 0;
        let mut map_size = MemoryMap::get_memory_map_size();
        let mut map: Vec<u8> = vec![0; map_size];
        status_to_result(unsafe {
            ((*bt).get_memory_map)(
                &mut map_size,
                map.as_mut_ptr() as *mut efi::MemoryDescriptor,
                &mut map_key,
                &mut desc_size,
                &mut desc_ver,
            )
        })
        .expect("Failed to get memory map");

        Self {
            map,
            map_size,
            map_key,
            desc_size,
            desc_ver,
        }
    }

    fn get_memory_map_size() -> usize {
        let bt = uefi::env::boot_services().unwrap().as_ptr() as *const efi::BootServices;

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
            panic!("Failed to get memory map size");
        }

        map_size + desc_size * 4
    }

    pub fn get_map_key(&self) -> usize {
        self.map_key
    }

    #[allow(dead_code)]
    pub fn iter(&self) -> MemoryMapIterator {
        MemoryMapIterator {
            map: self,
            offset: 0,
        }
    }
}

#[allow(dead_code)]
pub struct MemoryMapIterator<'a> {
    map: &'a MemoryMap,
    offset: usize,
}

impl<'a> Iterator for MemoryMapIterator<'a> {
    type Item = &'a MemoryDescriptor;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.map.map_size {
            return None;
        }
        let desc = unsafe { &*(self.map.map.as_ptr().add(self.offset) as *const MemoryDescriptor) };
        self.offset += self.map.desc_size;
        Some(desc)
    }
}

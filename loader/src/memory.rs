use super::status_to_result;
use r_efi::efi::{self, MemoryDescriptor};
use std::os::uefi;

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

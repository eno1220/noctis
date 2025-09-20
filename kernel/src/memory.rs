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

const MAX_MEMORY_REGION_LEN: usize = 128;

#[allow(dead_code)]
pub struct MemoryRegionArray {
    pub regions: [MemoryRegion; MAX_MEMORY_REGION_LEN],
    count: usize,
}

use core::ops::{Add, AddAssign, Sub, SubAssign};

#[allow(dead_code)]
pub trait Address:
    Copy
    + Clone
    + Eq
    + PartialEq
    + Ord
    + PartialOrd
    + Add<MSize>
    + AddAssign<MSize>
    + Sub<MSize>
    + SubAssign<MSize>
    + From<usize>
{
    fn from_ptr(addr: *const u8) -> Self;
    fn to_usize(&self) -> usize;
    fn to_ptr(&self) -> *const u8 {
        self.to_usize() as *const u8
    }
    fn to_ptr_mut(&self) -> *mut u8 {
        self.to_usize() as *mut u8
    }
    // Is MSize better...?
    fn align_up(&self, align: usize) -> Self;
}

macro_rules! impl_addrress {
    ($name:ident) => {
        impl Address for $name {
            fn to_usize(&self) -> usize {
                self.0
            }

            fn from_ptr(addr: *const u8) -> Self {
                $name(addr as usize)
            }

            fn align_up(&self, align: usize) -> Self {
                let offset = self.to_ptr().align_offset(align);
                $name::from_ptr(unsafe { self.to_ptr().add(offset) })
            }
        }

        impl Add<MSize> for $name {
            type Output = Self;
            fn add(self, rhs: MSize) -> Self::Output {
                Self(self.0 + rhs.0)
            }
        }

        impl AddAssign<MSize> for $name {
            fn add_assign(&mut self, rhs: MSize) {
                self.0 += rhs.0
            }
        }

        impl Sub<MSize> for $name {
            type Output = Self;
            fn sub(self, rhs: MSize) -> Self::Output {
                Self(self.0 - rhs.0)
            }
        }

        impl SubAssign<MSize> for $name {
            fn sub_assign(&mut self, rhs: MSize) {
                self.0 -= rhs.0
            }
        }

        impl From<usize> for $name {
            fn from(s: usize) -> Self {
                Self(s)
            }
        }
    };
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysAddr(usize);

impl PhysAddr {
    pub const fn new(addr: usize) -> Self {
        PhysAddr(addr)
    }
}

impl_addrress!(PhysAddr);

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtAddr(usize);

impl VirtAddr {
    pub const fn new(addr: usize) -> Self {
        VirtAddr(addr)
    }

    pub fn nth_level_table_index(&self, level: usize) -> usize {
        (self.0 >> (12 + ((level - 1) * 9))) & 0x1FF
    }
    #[allow(dead_code)]
    pub fn pml4_index(&self) -> usize {
        self.nth_level_table_index(4)
    }
    #[allow(dead_code)]
    pub fn pdpt_index(&self) -> usize {
        self.nth_level_table_index(3)
    }
    #[allow(dead_code)]
    pub fn pd_index(&self) -> usize {
        self.nth_level_table_index(2)
    }
    pub fn pt_index(&self) -> usize {
        self.nth_level_table_index(1)
    }
}

impl_addrress!(VirtAddr);

#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub struct MSize(usize);

impl MSize {
    pub const fn new(size: usize) -> Self {
        MSize(size)
    }

    pub fn to_usize(&self) -> usize {
        self.0
    }

    pub fn from_address<T: Address>(start_addr: T, end_addr: T) -> Self {
        assert!(start_addr <= end_addr);
        Self(end_addr.to_usize() - start_addr.to_usize())
    }

	#[allow(dead_code)]
    pub fn page_align_up(&self) -> Self {
        Self(self.0 + 0xFFF & !0xFFF)
    }
}

impl From<usize> for MSize {
    fn from(size: usize) -> Self {
        MSize::new(size)
    }
}

// IDEA: Supports KASLR
pub const KERNEL_CODE_BASE_VADDR: VirtAddr = VirtAddr::new(0xFFFF_FFFF_8000_0000);

// The base address of linear mapping of all physical
// memory in kernel address space.
// 512GB space mapped.
pub const LINER_MAPPING_BASE_VADDR: VirtAddr = VirtAddr::new(0xFFFF_8880_0000_0000);
pub const LINER_MAPPING_SIZE: MSize = MSize::new(0x8000000000);

const MAX_PHYS_ADDR: PhysAddr = PhysAddr::new(0xFFFF_FFFFFF_FFFF);

pub fn phys_to_virt(phys: PhysAddr) -> VirtAddr {
    debug_assert!(phys < MAX_PHYS_ADDR, "physical address is out of range");
    VirtAddr::new(phys.to_usize() + LINER_MAPPING_BASE_VADDR.to_usize())
}

pub fn virt_to_phys(virt: VirtAddr) -> PhysAddr {
    debug_assert!(
        virt >= LINER_MAPPING_BASE_VADDR,
        "virtual address is out of range"
    );
	if virt >= LINER_MAPPING_BASE_VADDR && virt <= LINER_MAPPING_BASE_VADDR + LINER_MAPPING_SIZE {
		PhysAddr::new(virt.to_usize() - LINER_MAPPING_BASE_VADDR.to_usize())
	} else if virt >= KERNEL_CODE_BASE_VADDR {
		PhysAddr::new(virt.to_usize() - KERNEL_CODE_BASE_VADDR.to_usize())
	} else {
		panic!("virt_to_phys: unsupported address region: {:#x}", virt.to_usize())
	}
}

pub const APIC_IO_START_ADDR: usize = 0xFEE0_0000;
pub const APIC_IO_SIZE: MSize = MSize::new(0x1000);

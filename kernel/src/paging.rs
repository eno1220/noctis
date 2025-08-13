use crate::{
    info,
    memlayout::{
        APIC_IO_SIZE, APIC_IO_START_ADDR, Address, LINER_MAPPING_BASE_VADDR,
        LINER_MAPPING_SIZE, MSize, PhysAddr, VirtAddr, phys_to_virt, virt_to_phys,
    },
    symbol_offsets,
    x86::write_cr3,
	println
};
use alloc::boxed::Box;
use core::fmt::Debug;
use core::{fmt, mem::MaybeUninit, pin::Pin};

const PAGE_SIZE: MSize = MSize::new(4096);

const PTE_ATTR_MASK: u64 = 0x7FFF_FFFF_FFFF_F000; // Mask for attributes
const PTE_ATTR_PRESENT: u64 = 1 << 0; // Page is present
const PTE_ATTR_WRITABLE: u64 = 1 << 1; // Page is writable
#[allow(dead_code)]
const PTE_ATTR_USER_ACCESSIBLE: u64 = 1 << 2; // Page is accessible by user mode
const PTE_ATTR_WRITE_THROUGH: u64 = 1 << 3; // Write-through caching
const PTE_ATTR_CACHE_DISABLED: u64 = 1 << 4; // Cache disabled
const PTE_ATTR_HUGE_PAGE: u64 = 1 << 7; // Huge Page
const PTE_ATTR_NOT_EXECUTABLE: u64 = 1 << 63; // Page is **not** executable

#[repr(u64)]
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub enum PageTableAttr {
    NotPresent = 0,
    ReadExecuteKernel = PTE_ATTR_PRESENT,
    ReadKernel = PTE_ATTR_PRESENT | PTE_ATTR_NOT_EXECUTABLE,
    ReadWriteExecuteKernel = PTE_ATTR_PRESENT | PTE_ATTR_WRITABLE,
    ReadWriteKernel = PTE_ATTR_PRESENT | PTE_ATTR_WRITABLE | PTE_ATTR_NOT_EXECUTABLE,
    ReadWriteKernel1GiB =
        PTE_ATTR_PRESENT | PTE_ATTR_WRITABLE | PTE_ATTR_NOT_EXECUTABLE | PTE_ATTR_HUGE_PAGE,
    ReadWriteKernelIO = PTE_ATTR_PRESENT
        | PTE_ATTR_WRITABLE
        | PTE_ATTR_WRITE_THROUGH
        | PTE_ATTR_CACHE_DISABLED
        | PTE_ATTR_NOT_EXECUTABLE,
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct PageTableEntry {
    value: u64,
}

impl PageTableEntry {
    fn get_bit(&self, bit: u8) -> bool {
        (self.value & (1 << bit)) != 0
    }
    #[allow(dead_code)]
    fn set_bit(&mut self, bit: u8, value: bool) {
        if value {
            self.value |= 1 << bit;
        } else {
            self.value &= !(1 << bit);
        }
    }

    fn is_present(&self) -> bool {
        self.get_bit(0)
    }
    fn is_writable(&self) -> bool {
        self.get_bit(1)
    }
    fn is_user_accessible(&self) -> bool {
        self.get_bit(2)
    }
    fn is_huge(&self) -> bool {
        self.get_bit(7)
    }
    fn is_executable(&self) -> bool {
        !self.get_bit(63)
    }

    fn format(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PageEntry(value: {:#x}, present: {}, writable: {}, user_accessible: {}, executable: {}, huge {})",
            self.value,
            self.is_present(),
            self.is_writable(),
            self.is_user_accessible(),
            self.is_executable(),
            self.is_huge()
        )
    }

    fn paddr(&self) -> PhysAddr {
        PhysAddr::new((self.value & PTE_ATTR_MASK) as usize)
    }
    fn set_entry(&mut self, paddr: PhysAddr, attr: PageTableAttr) -> Result<(), &'static str> {
        if paddr.to_usize() & !PTE_ATTR_MASK as usize != 0 {
            return Err("Physical address must be page-aligned");
        }
        self.value = (paddr.to_usize() as u64) | (attr as u64);
        Ok(())
    }

    fn next_node_mut(&mut self) -> Option<&mut PageTableNode> {
        if !self.is_present() {
            None
        } else {
            Some(unsafe { &mut *(phys_to_virt(self.paddr()).to_ptr_mut() as *mut PageTableNode) })
        }
    }
    fn alloc_next_level_table(&mut self) -> Result<&mut Self, &'static str> {
        if self.is_present() {
            Err("Next level table is already allocated")
        } else {
            // TODO: 物理メモリを取得するアロケータを実装し、そのアロケータからメモリを確保する
            let next: Box<PageTableNode> = Box::new(unsafe { MaybeUninit::zeroed().assume_init() });
            let phys_addr = virt_to_phys(VirtAddr::from_ptr(Box::into_raw(next) as *const u8));
            self.value =
                (phys_addr.to_usize() as u64) | PageTableAttr::ReadWriteExecuteKernel as u64;
            Ok(self)
        }
    }
    fn get_or_alloc_next_level_table(&mut self) -> Result<&mut PageTableNode, &'static str> {
        if !self.is_present() {
            self.alloc_next_level_table().unwrap();
        }
        Ok(self.next_node_mut().unwrap())
    }
}

impl fmt::Debug for PageTableEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.format(f)
    }
}

impl fmt::Display for PageTableEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.format(f)
    }
}

#[repr(align(4096))]
struct PageTableNode {
    entries: [PageTableEntry; 512],
}

pub struct PageTable {
    pml4: PageTableNode,
}

impl PageTable {
    pub fn new() -> Self {
        unsafe { MaybeUninit::zeroed().assume_init() }
    }
    fn map(
        &mut self,
        virt_start: VirtAddr,
        phys_start: PhysAddr,
        num_pages: usize,
        attr: PageTableAttr,
    ) -> Result<(), &'static str> {
        assert!(
            virt_start.to_usize() % PAGE_SIZE.to_usize() == 0,
            "Virtual address must be page-aligned"
        );
        assert!(
            phys_start.to_usize() % PAGE_SIZE.to_usize() == 0,
            "Physical address must be page-aligned"
        );

        if matches!(attr, PageTableAttr::ReadWriteKernel1GiB)
            && virt_start.to_usize() % (1 << 30) == 0
            && phys_start.to_usize() % (1 << 30) == 0
            && num_pages % (1 << 18) == 0
        {
            let pml4_index = virt_start.pml4_index();
            let pdpt_index = virt_start.pdpt_index();
            let pml4 = &mut self.pml4;
            let pdpt = pml4.entries[pml4_index].get_or_alloc_next_level_table()?;
            for i in 0..(num_pages / (1 << 18)) {
                let entry = &mut pdpt.entries[pdpt_index + i];
                entry.set_entry(PhysAddr::new(phys_start.to_usize() + i * (1 << 30)), attr)?;
            }
            return Ok(());
        }

        let mut node = &mut self.pml4;
        for level in (2..=4).rev() {
            let index = virt_start.nth_level_table_index(level);
            node = node.entries[index].get_or_alloc_next_level_table()?;
        }
        // TODO: VirtAddr, PhysAddrに対して四則演算用のtraitを実装する
        let mut vaddr = virt_start;
        let mut paddr = phys_start;
        let start_index = virt_start.pt_index();
        for i in 0..num_pages {
            let index = (start_index + i) % 512;
            if index == 0 && i != 0 {
                node = &mut self.pml4;
                for level in (2..=4).rev() {
                    let index = vaddr.nth_level_table_index(level);
                    node = node.entries[index].get_or_alloc_next_level_table()?;
                }
            }
            node.entries[index].set_entry(paddr, attr)?;
            vaddr += PAGE_SIZE;
            paddr += PAGE_SIZE;
        }
        Ok(())
    }
    pub fn create_mapping(
        &mut self,
        virt_start: VirtAddr,
        phys_start: PhysAddr,
        size: MSize,
        attr: PageTableAttr,
    ) -> Result<(), &'static str> {
        let num_pages = size.to_usize().div_ceil(PAGE_SIZE.to_usize()); // Or panic if size is not page-aligned...?
		println!("{:x?}", phys_start);
        self.map(virt_start, phys_start, num_pages, attr)
    }

    // 現時点ではカーネル空間のみ存在し、idleタスクのページテーブルを複製する
    // そのため再帰的な複製は行わず、単純にPML4のエントリをコピーする
    pub fn duplicate_kernel(&self) -> Pin<Box<PageTable>> {
        let mut new_table = Box::pin(PageTable::new());
        for (i, entry) in self.pml4.entries.iter().enumerate() {
            if entry.is_present() {
                let new_entry = &mut new_table.as_mut().pml4.entries[i];
                *new_entry = *entry;
            }
        }
        new_table
    }
}

pub fn init_paging() -> Pin<Box<PageTable>> {
    let mut page_table = Box::pin(PageTable::new());
    info!(
        "Paging initialized with PML4 at {:p}",
        &*page_table as *const PageTable
    );
    page_table
        .as_mut()
        .create_mapping(
            LINER_MAPPING_BASE_VADDR,
            PhysAddr::new(0),
            LINER_MAPPING_SIZE,
            PageTableAttr::ReadWriteKernel1GiB,
        )
        .expect("Failed to create direct mapping");
    page_table
        .as_mut()
        .create_mapping(
            symbol_offsets::__text(),
            virt_to_phys(symbol_offsets::__text()),
            MSize::from_address(symbol_offsets::__text(), symbol_offsets::__text_end()),
            PageTableAttr::ReadExecuteKernel,
        )
        .expect("Failed to .text area mapping");
    page_table
        .as_mut()
        .create_mapping(
            symbol_offsets::__rodata(),
            virt_to_phys(symbol_offsets::__rodata()),
            MSize::from_address(symbol_offsets::__rodata(), symbol_offsets::__rodata_end()),
            PageTableAttr::ReadKernel,
        )
        .expect("Failed to .rodata area mapping");
    page_table
        .as_mut()
        .create_mapping(
            symbol_offsets::__data(),
            virt_to_phys(symbol_offsets::__data()),
            MSize::from_address(symbol_offsets::__data(), symbol_offsets::__bss_end()),
            PageTableAttr::ReadWriteKernel,
        )
        .expect("Failed to .data .bss area mapping");
    page_table
        .as_mut()
        .create_mapping(
            VirtAddr::new(APIC_IO_START_ADDR),
            PhysAddr::new(APIC_IO_START_ADDR),
            APIC_IO_SIZE,
            PageTableAttr::ReadWriteKernelIO,
        )
        .expect("Failed to create kernel I/O mapping");
    write_cr3(virt_to_phys(VirtAddr::new(
        &mut page_table.pml4 as *mut _ as usize,
    )));
    page_table
}

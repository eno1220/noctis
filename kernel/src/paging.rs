use crate::{info, symbol_offsets, x86::write_cr3};
use alloc::boxed::Box;
use core::fmt::Debug;
use core::{fmt, mem::MaybeUninit, pin::Pin};

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct PhysAddr(usize);

impl PhysAddr {
    pub fn new(addr: usize) -> Self {
        PhysAddr(addr)
    }
    pub fn as_usize(&self) -> usize {
        self.0
    }
    pub fn as_u64(&self) -> u64 {
        self.0 as u64
    }
}

impl fmt::Debug for PhysAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PhysAddr({:#x})", self.0)
    }
}

impl From<usize> for PhysAddr {
    fn from(addr: usize) -> Self {
        PhysAddr::new(addr)
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct VirtAddr(usize);

impl VirtAddr {
    pub fn new(addr: usize) -> Self {
        VirtAddr(Self::canonicalize(addr as u64) as usize)
    }
    fn canonicalize(addr: u64) -> u64 {
        if addr & (1 << 47) != 0 {
            addr | 0xFFFF_0000_0000_0000
        } else {
            addr & 0x0000_FFFF_FFFF_FFFF
        }
    }
    pub fn as_usize(&self) -> usize {
        self.0
    }
    #[allow(dead_code)]
    pub fn as_u64(&self) -> u64 {
        self.0 as u64
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

    pub fn add(&self, offset: usize) -> Self {
        VirtAddr::new(self.0.wrapping_add(offset))
    }
}

impl From<usize> for VirtAddr {
    fn from(addr: usize) -> Self {
        VirtAddr::new(addr)
    }
}

impl fmt::Debug for VirtAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VirtAddr({:#x})", self.0)
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub struct MSize(usize);

impl MSize {
    pub fn new(size: usize) -> Self {
        MSize(size)
    }
    pub fn as_usize(&self) -> usize {
        self.0
    }
}

impl From<usize> for MSize {
    fn from(size: usize) -> Self {
        MSize::new(size)
    }
}

// TODO: メモリレイアウト由来のアドレスを別ファイルへ移動する
pub const KERNEL_VADDR_BASE: usize = 0xFFFFFFFF80000000;
const KERNEL_DIRECT_START: usize = 0xffff888000000000;
const KERNEL_DIRECT_SIZE: usize = 0x8000000000;
const PAGE_SIZE: usize = 4096; // 4 KiB

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
        if paddr.as_usize() & !PTE_ATTR_MASK as usize != 0 {
            return Err("Physical address must be page-aligned");
        }
        self.value = paddr.as_u64() | (attr as u64);
        Ok(())
    }

    fn next_node_mut(&mut self) -> Option<&mut PageTableNode> {
        if !self.is_present() {
            None
        } else {
            Some(unsafe { &mut *(self.paddr().as_usize() as *mut PageTableNode) })
        }
    }
    fn alloc_next_level_table(&mut self) -> Result<&mut Self, &'static str> {
        if self.is_present() {
            Err("Next level table is already allocated")
        } else {
            // 現在は恒等マッピングを仮定しているためこれで良いが、
            // 将来的には物理アドレスを割り当てる必要がある
            let next: Box<PageTableNode> = Box::new(unsafe { MaybeUninit::zeroed().assume_init() });
            self.value = Box::into_raw(next) as u64 | PageTableAttr::ReadWriteExecuteKernel as u64;
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
            virt_start.as_usize() % PAGE_SIZE == 0,
            "Virtual address must be page-aligned"
        );
        assert!(
            phys_start.as_usize() % PAGE_SIZE == 0,
            "Physical address must be page-aligned"
        );

        if matches!(attr, PageTableAttr::ReadWriteKernel1GiB)
            && virt_start.as_usize() % (1 << 30) == 0
            && phys_start.as_usize() % (1 << 30) == 0
            && num_pages % (1 << 18) == 0
        {
            let pml4_index = virt_start.pml4_index();
            let pdpt_index = virt_start.pdpt_index();
            let pml4 = &mut self.pml4;
            let pdpt = pml4.entries[pml4_index].get_or_alloc_next_level_table()?;
            for i in 0..(num_pages / (1 << 18)) {
                let entry = &mut pdpt.entries[pdpt_index + i];
                entry.set_entry(PhysAddr(phys_start.as_usize() + i * (1 << 30)), attr)?;
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
            vaddr = vaddr.add(PAGE_SIZE);
            paddr = PhysAddr::new(paddr.as_usize() + PAGE_SIZE);
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
        let num_pages = size.as_usize().div_ceil(PAGE_SIZE); // Or panic if size is not page-aligned...?
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
            VirtAddr::new(KERNEL_DIRECT_START),
            PhysAddr::new(0),
            MSize::new(KERNEL_DIRECT_SIZE),
            PageTableAttr::ReadWriteKernel1GiB,
        )
        .expect("Failed to create direct mapping");
    page_table
        .as_mut()
        .create_mapping(
            VirtAddr::new(0x0000),
            PhysAddr::new(0x0000),
            MSize::new(1024 * 1024 * 1024), // 1 GiB (QEMU起動時のメモリサイズに合わせる)
            PageTableAttr::ReadWriteKernel,
        )
        .expect("Failed to create initial mapping");
    page_table
        .as_mut()
        .create_mapping(
            VirtAddr::new(symbol_offsets::__text()),
            PhysAddr::new(symbol_offsets::__text() - KERNEL_VADDR_BASE),
            MSize::new(symbol_offsets::__text_end() - symbol_offsets::__text()),
            PageTableAttr::ReadExecuteKernel,
        )
        .expect("Failed to .text area mapping");
    page_table
        .as_mut()
        .create_mapping(
            VirtAddr::new(symbol_offsets::__rodata()),
            PhysAddr::new(symbol_offsets::__rodata() - KERNEL_VADDR_BASE),
            MSize::new(symbol_offsets::__rodata_end() - symbol_offsets::__rodata()),
            PageTableAttr::ReadKernel,
        )
        .expect("Failed to .rodata area mapping");
    page_table
        .as_mut()
        .create_mapping(
            VirtAddr::new(0xFEE0_0000),
            PhysAddr::new(0xFEE0_0000),
            MSize::new(0x1000), // 4 KiB for I/O APIC
            PageTableAttr::ReadWriteKernelIO,
        )
        .expect("Failed to create kernel I/O mapping");
    write_cr3(&mut page_table.pml4 as *mut _ as usize);
    page_table
}

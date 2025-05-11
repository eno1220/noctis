use alloc::boxed::Box;
use bitfield_struct::bitfield;
use core::arch::asm;
use core::mem::size_of;
use core::pin::Pin;

#[bitfield(u64)]
struct GdtSegemntDescriptor {
    limit_low: u16,
    #[bits(24)]
    base_low: u32,
    accessed: bool,
    rw: bool,
    dc: bool,
    executable: bool,
    #[bits(1)]
    descriptor_type: u8,
    #[bits(2)]
    dpl: u8,
    present: bool,
    #[bits(4)]
    limit_high: u8,
    #[bits(1)]
    avl: u8,
    long: bool,
    db: bool,
    #[bits(1)]
    granularity: u8,
    base_high: u8,
}

const _: () = assert!(size_of::<GdtSegemntDescriptor>() == 8);

impl GdtSegemntDescriptor {
    const fn null() -> Self {
        unsafe { core::mem::zeroed() }
    }
    fn create(
        rw: bool,
        dc: bool,
        executable: bool,
        base: u32,
        limit: u32,
        dpl: u8,
        granularity: u8,
    ) -> Self {
        Self::default()
            .with_limit_low((limit & 0xffff) as u16)
            .with_base_low(base & 0xffffff)
            .with_accessed(false)
            .with_rw(rw)
            .with_dc(dc)
            .with_executable(executable)
            .with_descriptor_type(0b01) // code or data segment
            .with_dpl(dpl & 0b11)
            .with_present(true)
            .with_limit_high(((limit >> 16) & 0xff) as u8)
            .with_avl(1)
            .with_long(executable)
            .with_db(!executable)
            .with_granularity(granularity & 0b1)
            .with_base_high((base >> 24) as u8)
    }
}

#[repr(C, packed)]
struct Tss64Inner {
    _reserved0: u32,
    _rsp: [u64; 3],
    _reserved1: u64,
    _ist: [u64; 7],
    _reserved2: u64,
    _io_map_base: u16,
    _reserved3: u16,
}

const _: () = assert!(size_of::<Tss64Inner>() == 104);

struct Tss64 {
    inner: Pin<Box<Tss64Inner>>,
}

impl Tss64 {
    pub fn phys_addr(&self) -> u64 {
        self.inner.as_ref().get_ref() as *const Tss64Inner as u64
    }
    unsafe fn allocate_tss_memory() -> u64 {
        const TSS_SIZE: usize = 64 * 1024;
        let stack = Box::new([0u8; TSS_SIZE]);
        let rsp = unsafe { stack.as_ptr().add(TSS_SIZE) as u64 };
        core::mem::forget(stack);
        rsp
    }
    pub fn new() -> Self {
        let rsp0 = unsafe { Self::allocate_tss_memory() };
        let mut ist = [0u64; 7];
        for i in ist.iter_mut() {
            *i = unsafe { Self::allocate_tss_memory() };
        }
        let tss64 = Tss64Inner {
            _reserved0: 0,
            _rsp: [rsp0, 0, 0],
            _reserved1: 0,
            _ist: ist,
            _reserved2: 0,
            _io_map_base: 0,
            _reserved3: 0,
        };

        Self {
            inner: Box::pin(tss64),
        }
    }
}

impl Drop for Tss64 {
    fn drop(&mut self) {
        panic!("TSS memory deallocation not implemented");
    }
}

#[bitfield(u128)]
struct TssDescriptor {
    limit_low: u16,
    #[bits(24)]
    base_low: u32,
    #[bits(4)]
    r#type: u8,
    #[bits(1)]
    desc_type: u8,
    #[bits(2)]
    dpl: u8,
    present: bool,
    #[bits(4)]
    limit_high: u8,
    #[bits(1)]
    avl: u8,
    long: bool,
    #[bits(1)]
    db: u8,
    #[bits(1)]
    granularity: u8,
    #[bits(40)]
    base_high: u64,
    _reserved: u32,
}

const _: () = assert!(size_of::<TssDescriptor>() == 16);

impl TssDescriptor {
    fn create(base: u64) -> Self {
        Self::default()
            .with_limit_low((size_of::<Tss64Inner>() & 0xffff) as u16)
            .with_base_low((base & 0xffffff) as u32)
            .with_type(0b1001) // TSS
            .with_desc_type(0b00) // system segment
            .with_dpl(0b00) // kernel
            .with_present(true)
            .with_limit_high(((size_of::<Tss64Inner>() >> 16) & 0xff) as u8)
            .with_avl(0)
            .with_long(true)
            .with_db(0)
            .with_granularity(0b01) // 4K granularity
            .with_base_high(base >> 24)
    }
}

pub const KERNEL_CODE_SEGMENT: u16 = 1 << 3;
pub const KERNEL_DATA_SEGMENT: u16 = 2 << 3;
pub const TSS64_SEGMENT_SELECTOR: u16 = 3 << 3;

#[repr(C, packed)]
struct Gdt {
    null_segment: GdtSegemntDescriptor,
    kernel_code_segment: GdtSegemntDescriptor,
    kernel_data_segment: GdtSegemntDescriptor,
    tss_segment: TssDescriptor,
}

const _: () = assert!(size_of::<Gdt>() == 40);

#[allow(dead_code)]
pub struct GdtWrapper {
    inner: Pin<Box<Gdt>>,
    tss64: Tss64,
}

#[repr(C, packed)]
struct GdtRegister {
    limit: u16,
    base: *const Gdt,
}

impl GdtWrapper {
    pub fn load(&self) {
        let gdt_register = GdtRegister {
            limit: (size_of::<Gdt>() - 1) as u16,
            base: self.inner.as_ref().get_ref() as *const Gdt,
        };
        unsafe {
            asm!(
                "lgdt [{}]",
                in(reg) &gdt_register,
            );
            asm!(
                "ltr {0:x}",
                in(reg) TSS64_SEGMENT_SELECTOR,
            );
        }
    }
}

impl Default for GdtWrapper {
    fn default() -> Self {
        let tss64 = Tss64::new();
        let gdt = Gdt {
            null_segment: GdtSegemntDescriptor::null(),
            kernel_code_segment: GdtSegemntDescriptor::create(
                true, false, true, 0, 0xfffff, // 2^20 - 1
                0, 0b1,
            ),
            kernel_data_segment: GdtSegemntDescriptor::create(
                true, false, false, 0, 0xfffff, // 2^20 - 1
                0, 0b1,
            ),
            tss_segment: TssDescriptor::create(tss64.phys_addr()),
        };
        let gdt = Box::pin(gdt);
        Self { inner: gdt, tss64 }
    }
}

fn load_kernel_data_segment() {
    unsafe {
        asm!(
            "mov di, {:x}",
            "mov ds, di",
            "mov es, di",
            "mov fs, di",
            "mov gs, di",
            "mov ss, di",
            in(reg) KERNEL_DATA_SEGMENT,
        );
    }
}

// csレジスタのみlfar-jumpする必要がある
fn load_kernel_code_segment() {
    unsafe {
        asm!(
            "lea rax, [rip + 2f]",
            "push cx",
            "push rax",
            "ljmp [rsp]",
            "2:",
            "add rsp, 8 + 2",
            in("cx") KERNEL_CODE_SEGMENT,
        )
    }
}

pub fn init_gdt() -> GdtWrapper {
    let gdt = GdtWrapper::default();
    gdt.load();
    load_kernel_data_segment();
    load_kernel_code_segment();
    gdt
}

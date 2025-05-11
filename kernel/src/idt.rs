use crate::{error, gdt, info};
use alloc::boxed::Box;
use bitfield_struct::bitfield;
use core::arch::{asm, global_asm, naked_asm};
use core::mem::size_of;
use core::pin::Pin;
use core::{fmt, u128};

#[repr(C)]
#[derive(Debug)]
struct InterruptContext {
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}

const _: () = assert!(size_of::<InterruptContext>() == 8 * 5);

#[repr(C)]
#[derive(Debug)]
struct InterruptRegisters {
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    rbp: u64,
    rsp: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
}

const _: () = assert!(size_of::<InterruptRegisters>() == 8 * 16);

#[repr(C)]
struct InterruptStackFrame {
    registers: InterruptRegisters,
    vector: u64,
    error_code: u64,
    context: InterruptContext,
}

impl fmt::Debug for InterruptStackFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "InterruptStackFrame {{\n\
            registers: {:#010x?},\n\
            vector: {},\n\
            error_code: {},\n\
            context: {:#010x?}\n\
        }}",
            self.registers, self.vector, self.error_code, self.context
        )
    }
}

macro_rules! interrupt_entry_without_ecode {
    ($index:literal) => {
        global_asm!(concat!(
            ".global interrupt_entry_",
            stringify!($index),
            "\n",
            "interrupt_entry_",
            stringify!($index),
            ":\n",
            "push 0\n", // Push a dummy error code
            "push ",
            stringify!($index),
            "\n", // Push the interrupt vector
            "jmp interrupt_handler_common\n"
        ));
    };
}

macro_rules! interrupt_entry_with_ecode {
    ($index:literal) => {
        global_asm!(concat!(
            ".global interrupt_entry_",
            stringify!($index),
            "\n",
            "interrupt_entry_",
            stringify!($index),
            ":\n",
            "push ",
            stringify!($index),
            "\n", // Push the interrupt vector
            "jmp interrupt_handler_common\n"
        ));
    };
}

interrupt_entry_without_ecode!(3);
interrupt_entry_without_ecode!(6);
interrupt_entry_with_ecode!(13);

unsafe extern "x86-interrupt" {
    fn interrupt_entry_3();
    fn interrupt_entry_6();
    fn interrupt_entry_13();
}

#[allow(unused)]
#[unsafe(naked)]
#[unsafe(no_mangle)]
unsafe fn interrupt_handler_common() {
    naked_asm!(
        // Save registers
        "push r15",
        "push r14",
        "push r13",
        "push r12",
        "push r11",
        "push r10",
        "push r9",
        "push r8",
        "push rdi",
        "push rsi",
        "push rbp",
        "push rsp",
        "push rbx",
        "push rdx",
        "push rcx",
        "push rax",
        "push rsp",
        "pop rdi", // Save the stack pointer to rdi
        "push rsp",
        "push [rsp]",
        "and rsp, 0xfffffffffffffff0", // Align the stack pointer to 16 bytes
        "call interrupt_handler",
        "mov rsp, [rsp + 8]", // Restore the original stack pointer
        // Restore registers
        "pop rax",
        "pop rcx",
        "pop rdx",
        "pop rbx",
        "pop rsp",
        "pop rbp",
        "pop rsi",
        "pop rdi",
        "pop r8",
        "pop r9",
        "pop r10",
        "pop r11",
        "pop r12",
        "pop r13",
        "pop r14",
        "pop r15",
        // Return from the interrupt
        "add rsp, 0x10",
        "iretq"
    );
}

#[unsafe(no_mangle)]
extern "C" fn interrupt_handler(stack_frame: &InterruptStackFrame) {
    info!("Interrupt occurred: {:?}", stack_frame);
    match stack_frame.vector {
        // Breakpoint exception
        3 => {
            error!("Breakpoint exception");
            return;
        }
        // Invalid opcode exception
        6 => {
            error!("Invalid opcode exception");
        }
        // General protection fault
        13 => {
            error!("General protection fault");
            let rip = stack_frame.context.rip;
            error!("RIP: {rip:#018x}");
        }
        _ => {
            error!("Unhandled interrupt: {}", stack_frame.vector);
        }
    }
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[unsafe(no_mangle)]
extern "x86-interrupt" fn interrupt_handler_unimplemented() {
    panic!("Unimplemented interrupt handler");
}

#[bitfield(u128)]
struct IdtDescriptor {
    offset_low: u16,
    segment_selector: u16,
    #[bits(3)]
    ist: u8,
    #[bits(5)]
    reserved1: u8,
    #[bits(4)]
    gate_type: u8,
    #[bits(1)]
    reserved2: u8,
    #[bits(2)]
    dpl: u8,
    #[bits(1)]
    present: bool,
    offset_middle: u16,
    offset_high: u32,
    reserved3: u32,
}

const _: () = assert!(size_of::<IdtDescriptor>() == 16);

impl IdtDescriptor {
    fn create(
        segment_selector: u16,
        ist_index: u8,
        gate_type: u8,
        dpl: u8,
        f: unsafe extern "x86-interrupt" fn(),
    ) -> Self {
        let f = f as u64;
        Self::default()
            .with_offset_low((f & 0xffff) as u16)
            .with_segment_selector(segment_selector)
            .with_ist(ist_index)
            .with_gate_type(gate_type & 0b1111)
            .with_dpl(dpl & 0b11)
            .with_present(true)
            .with_offset_middle(((f >> 16) & 0xffff) as u16)
            .with_offset_high((f >> 32) as u32)
    }
}

const IDT_DPL_0: u8 = 0b00;
const IDT_DPL_3: u8 = 0b11;
const IDT_GATE_TYPE_INTGATE: u8 = 0b1110;

#[repr(C, packed)]
pub struct Idt {
    entries: Pin<Box<[IdtDescriptor; 0x100]>>,
}

impl Idt {
    pub fn new(segment_selector: u16) -> Self {
        let mut entries = [IdtDescriptor::create(
            segment_selector,
            1,
            IDT_GATE_TYPE_INTGATE,
            IDT_DPL_0,
            interrupt_handler_unimplemented,
        ); 0x100];
        entries[3] = IdtDescriptor::create(
            segment_selector,
            1,
            IDT_GATE_TYPE_INTGATE,
            IDT_DPL_3,
            interrupt_entry_3,
        );
        entries[6] = IdtDescriptor::create(
            segment_selector,
            1,
            IDT_GATE_TYPE_INTGATE,
            IDT_DPL_0,
            interrupt_entry_6,
        );
        entries[13] = IdtDescriptor::create(
            segment_selector,
            1,
            IDT_GATE_TYPE_INTGATE,
            IDT_DPL_0,
            interrupt_entry_13,
        );
        let entries = Box::pin(entries);
        let register = IdtRegister {
            limit: (entries.len() * size_of::<IdtDescriptor>() - 1) as u16,
            base: entries.as_ptr(),
        };
        unsafe {
            asm!(
                "lidt [{}]",
                in(reg) &register,
                options(nostack),
            );
        }
        Self { entries }
    }
}

#[repr(C, packed)]
struct IdtRegister {
    limit: u16,
    base: *const IdtDescriptor,
}

const _: () = assert!(size_of::<IdtRegister>() == 10);

pub fn init_idt() -> Idt {
    let idt = Idt::new(gdt::KERNEL_CODE_SEGMENT);
    idt
}

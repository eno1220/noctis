use core::arch::asm;

pub fn write_io(port: u16, value: u8) {
    unsafe {
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
        );
    }
}

pub fn read_io(port: u16) -> u8 {
    let mut value: u8;
    unsafe {
        asm!(
            "in al, dx",
            in("dx") port,
            out("al") value,
        );
    }
    value
}

#[allow(dead_code)]
pub fn disable_interrupts() {
    unsafe {
        asm!("cli", options(nostack),);
    }
}

pub fn enable_interrupts() {
    unsafe {
        asm!("sti", options(nostack),);
    }
}

pub fn write_cr3(value: usize) {
    unsafe {
        asm!(
            "mov cr3, {}",
            in(reg) value,
            options(nostack),
        );
    }
}

macro_rules! make_read_reg {
	($fn_name:ident, $reg:tt) => {
		#[allow(dead_code)]
		pub fn $fn_name() -> usize {
			let value: usize;
			unsafe {
				asm!(
					concat!("mov {}, ", $reg),
					out(reg) value,
					options(nostack),
				);
			}
			value
		}
	};
}

make_read_reg!(read_rax, "rax");
make_read_reg!(read_rbx, "rbx");
make_read_reg!(read_rcx, "rcx");
make_read_reg!(read_rdx, "rdx");
make_read_reg!(read_rsi, "rsi");
make_read_reg!(read_rdi, "rdi");
make_read_reg!(read_rbp, "rbp");
make_read_reg!(read_rsp, "rsp");
make_read_reg!(read_r8, "r8");
make_read_reg!(read_r9, "r9");
make_read_reg!(read_r10, "r10");
make_read_reg!(read_r11, "r11");
make_read_reg!(read_r12, "r12");
make_read_reg!(read_r13, "r13");
make_read_reg!(read_r14, "r14");
make_read_reg!(read_r15, "r15");
make_read_reg!(read_cr0, "cr0");
make_read_reg!(read_cr2, "cr2");
make_read_reg!(read_cr3, "cr3");
make_read_reg!(read_cr4, "cr4");

#[allow(dead_code)]
pub fn read_rip() -> usize {
    let rip: usize;
    unsafe {
        asm!(
            "lea {}, [rip]",
            out(reg) rip,
            options(nostack),
        );
    }
    rip
}

#[allow(dead_code)]
pub fn read_rflags() -> usize {
    let rflags: usize;
    unsafe {
        asm!(
            "pushfq",
            "pop {}",
            out(reg) rflags,
            options(nostack),
        );
    }
    rflags
}

use crate::x86;

const IO_ADDR_COM1: u16 = 0x3F8;

#[allow(dead_code)]
const IO_BAUD_DIVISOR: u16 = 0x01;

pub struct Uart {
    base: u16,
}

impl Uart {
    pub fn new(base: u16) -> Self {
        Uart { base }
    }

    pub fn init(&self) {
        x86::write_io(self.base + 1, 0x00);
        x86::write_io(self.base + 3, 0x80);
        x86::write_io(self.base + 0, (IO_BAUD_DIVISOR & 0xFF) as u8);
        x86::write_io(self.base + 1, (IO_BAUD_DIVISOR >> 8) as u8);
        x86::write_io(self.base + 3, 0x03);
        x86::write_io(self.base + 2, 0xC7);
        x86::write_io(self.base + 4, 0x0B);
    }

    pub fn write(&self, byte: char) {
        while x86::read_io(self.base + 5) & 0x20 == 0 {
            core::hint::spin_loop();
        }
        x86::write_io(self.base, byte as u8);
    }

    pub fn write_str(&self, s: &str) {
        for byte in s.bytes() {
            self.write(byte as char);
        }
    }
}

impl core::fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let serial = Self::default();
        serial.write_str(s);
        Ok(())
    }
}

impl Default for Uart {
    fn default() -> Self {
        Uart::new(IO_ADDR_COM1)
    }
}

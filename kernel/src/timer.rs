use crate::spin::SpinLock;

pub struct LocalApicTimer {
    base: usize,
    pub count: u32,
}

impl LocalApicTimer {
    pub const BASE: usize = 0xFEE00000;
    const TIMER: usize = 0x320;
    const TIMER_DIV: usize = 0x3E0;
    const TIMER_INIT_COUNT: usize = 0x380;
    const END_OF_INTERRUPT: usize = 0xB0;

    pub const fn new() -> Self {
        LocalApicTimer {
            base: Self::BASE,
            count: 0,
        }
    }

    fn register(&self, offset: usize) -> *mut u32 {
        (self.base + offset) as *mut u32
    }

    pub fn init(&self) {
        unsafe {
            core::ptr::write_volatile(self.register(Self::TIMER_DIV), 0b110);
            core::ptr::write_volatile(self.register(Self::TIMER_INIT_COUNT), 0x1000000);
            // periodic interrupt
            // call interrupt handler 0x2a(42)
            core::ptr::write_volatile(self.register(Self::TIMER), (0b010 << 16) | 0x2a);
        }
    }

    fn increment_count(&mut self) {
        self.count += 1;
        if self.count == 0 {
            self.count = 1;
        }
    }

    fn notify_end_of_interrupt(&self) {
        unsafe {
            core::ptr::write_volatile(self.register(Self::END_OF_INTERRUPT), 0);
        }
    }
}

static LOCAL_APIC_TIMER: SpinLock<LocalApicTimer> = SpinLock::new(LocalApicTimer::new());

pub fn init_timer() {
    LOCAL_APIC_TIMER.lock().init();
}

pub fn get_count() -> u32 {
    LOCAL_APIC_TIMER.lock().count
}

pub fn increment_count() {
    LOCAL_APIC_TIMER.lock().increment_count();
}

pub fn notify_end_of_interrupt() {
    LOCAL_APIC_TIMER.lock().notify_end_of_interrupt();
}

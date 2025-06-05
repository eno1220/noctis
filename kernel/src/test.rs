use crate::qemu::{QemuExitCode, exit_qemu};
use crate::{error, info, print, println};
use core::panic::PanicInfo;

pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        print!("[TEST] {} >>> ", core::any::type_name::<T>());
        self();
        println!("[PASS]");
    }
}

#[cfg(test)]
pub fn test_runner(tests: &[&dyn Testable]) -> ! {
    info!("Runnning {} tests", tests.len());
    for test in tests {
        test.run();
    }
    info!("Completed {} tests", tests.len());
    exit_qemu(QemuExitCode::Sucess);
}

#[cfg(test)]
#[panic_handler]
fn panic(panic_info: &PanicInfo) -> ! {
    error!("!!!!! Panic During Test !!!!!");
    error!("Panic info: {:?}", panic_info);
    exit_qemu(QemuExitCode::Fail);
}

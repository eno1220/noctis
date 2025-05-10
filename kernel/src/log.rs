use crate::uart::Uart;
use core::fmt;

#[allow(dead_code)]
pub fn print(args: core::fmt::Arguments) {
    let mut serial = Uart::default();
    fmt::Write::write_fmt(&mut serial, args).unwrap();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::log::print(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! println {
    () => {
        $crate::log::print(format_args!("\n"));
    };
    ($($arg:tt)*) => {
        $crate::log::print(format_args!(concat!($($arg)*, "\n")));
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::log::print(format_args!("[WARN] {}:{:<3}: {}\n", file!(), line!(), format_args!($($arg)*)));
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::log::print(format_args!("[ERROR] {}:{:<3}: {}\n", file!(), line!(), format_args!($($arg)*)));
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::log::print(format_args!("[INFO] {}:{:<3}: {}\n", file!(), line!(), format_args!($($arg)*)));
    };
}

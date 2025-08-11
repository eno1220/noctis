use crate::uart::Uart;
use core::fmt;

pub const COLOR_RESET: &str = "\x1b[0m";
pub const COLOR_CYAN: &str = "\x1b[36m";
#[allow(dead_code)]
pub const COLOR_YELLOW: &str = "\x1b[33m";
pub const COLOR_RED: &str = "\x1b[31m";

// TODO: FIXME
// ログ出力関数が呼ばれる度にserialを初期化しているのが問題ないか確認する
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
        $crate::log::print(format_args!("{}\n", format_args!($($arg)*)));
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::log::print(format_args!("{}[WARN]{} {}:{:<3}: {}\n", $crate::log::COLOR_YELLOW, crate::log::COLOR_RESET, file!(), line!(), format_args!($($arg)*)));
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::log::print(format_args!("{}[ERROR]{} {}:{:<3}: {}\n", $crate::log::COLOR_RED, crate::log::COLOR_RESET, file!(), line!(), format_args!($($arg)*)));
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::log::print(format_args!("{}[INFO]{} {}:{:<3}: {}\n", $crate::log::COLOR_CYAN, crate::log::COLOR_RESET, file!(), line!(), format_args!($($arg)*)));
    };
}

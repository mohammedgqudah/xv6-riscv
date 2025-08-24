//! Rust bindings for xv6 kernel functions.

#![no_std]

use core::{
    fmt::{self, Write},
    panic::PanicInfo,
};
#[allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::undocumented_unsafe_blocks,
    clippy::missing_safety_doc,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code,
    improper_ctypes,
    unsafe_op_in_unsafe_fn
)]
pub mod bindings;
pub mod mutex;
pub mod page;
pub use page::{DeviceOwned, HostOwned, KernelBuffer, Page};

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    println!("{}", _info.message());
    unsafe {
        bindings::panic(c"rust panic".as_ptr() as *mut _);
    }
}

struct Xv6Printer;
impl Write for Xv6Printer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            unsafe {
                bindings::consputc(c as i32);
            }
        }
        Ok(())
    }
}

pub fn _print(args: fmt::Arguments) {
    let mut printer = Xv6Printer;
    printer.write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        $crate::_print(format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! println {
    () => {
        $crate::print!("\n")
    };
    ($($arg:tt)*) => {{
        $crate::print!("{}\n", format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! dbg {
    () => {
        $crate::println!("[{}:{}:{}]", file!(), line!(), column!())
    };
    ($val:expr $(,)?) => {
        match $val {
            tmp => {
                $crate::println!("[{}:{}:{}] {} = {:#?}",
                    file!(),
                    line!(),
                    column!(),
                    stringify!($val),
                    &&tmp as &dyn core::fmt::Debug,
                );
                tmp
            }
        }
    };
    ($($val:expr),+ $(,)?) => {
        ($($crate::dbg!($val)),+,)
    };
}

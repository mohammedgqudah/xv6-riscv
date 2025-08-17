//! Rust bindings for xv6 kernel functions.
//!
//! Instead of manually defining these, I should use something like bindgen.

#![no_std]

use core::{
    fmt::{self, Write},
    panic::PanicInfo,
};
pub mod bindings;
pub mod mutex;

#[repr(packed)]
pub struct Page(*mut core::ffi::c_char);

impl Page {
    pub fn new(ptr: *mut core::ffi::c_char) -> Self {
        Self(ptr)
    }

    pub fn free(self) {
        kfree(self);
    }
}

#[repr(packed)]
pub struct KernelBuffer {
    pub page: Page,
    pub length: usize,
}

impl KernelBuffer {
    pub fn new(ptr: *mut core::ffi::c_char, len: usize) -> Self {
        Self {
            page: Page::new(ptr),
            length: len,
        }
    }
}

/// Free a 4069-byte page.
pub fn kfree(page: Page) {
    unsafe {
        bindings::kfree(page.0 as _);
    }
}

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
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


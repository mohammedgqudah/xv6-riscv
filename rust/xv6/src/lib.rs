//! Rust bindings for xv6 kernel functions.
//!
//! Instead of manually defining these, I should use something like bindgen.

#![no_std]

use core::panic::PanicInfo;
pub mod bindings;

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    unsafe {
        bindings::panic(c"rust panic".as_ptr() as *mut _);
    }
}

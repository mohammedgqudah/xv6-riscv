//! Rust bindings for xv6 kernel functions.
//!
//! Instead of manually defining these, I should use something like bindgen.

#![no_std]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));


use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub extern "C" fn rs_hello() {
    unsafe {
        printf(c"yay!\n".as_ptr() as *mut _);
    }
}

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    unsafe {
        panic(c"rust panic".as_ptr() as *mut _);
    }
}

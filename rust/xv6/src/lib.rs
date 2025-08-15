#![no_std]
//! Rust bindings for xv6 kernel functions.
//!
//! Instead of manually defining these, I should use something like bindgen.

use core::panic::PanicInfo;
unsafe extern "C" {
    // Allocate one 4096-byte page of physical memory.
    // Returns a pointer that the kernel can use.
    // Returns 0 if the memory cannot be allocated.
    unsafe fn kalloc() -> *mut core::ffi::c_void;
    // Free the page of physical memory pointed at by pa,
    // which normally should have been returned by a
    // call to kalloc().  (The exception is when
    // initializing the allocator; see kinit above.)
    unsafe fn kfree(pa: *mut core::ffi::c_void);
    unsafe fn printf(fmt: *const u8, ...) -> i32;
    safe fn panic(msg: *const u8) -> !;
}

#[unsafe(no_mangle)]
pub extern "C" fn rs_hello() {
    unsafe {
        printf(c"yay!\n".as_ptr());
    }
}

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    panic(c"rust panic".as_ptr());
}

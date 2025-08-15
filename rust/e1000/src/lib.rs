#![no_std]

#[unsafe(no_mangle)]
pub extern "C" fn rs_hello() {
    unsafe {
        xv6::bindings::printf(c"hello from e7000\n".as_ptr() as *mut _);
    }
}

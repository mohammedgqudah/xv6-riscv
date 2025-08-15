#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use core::ffi::c_void;

use crate::KernelBuffer;
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

impl tx_desc {
    pub fn is_done(&self) -> bool {
        (self.status & E1000_TXD_STAT_DD as u8) == E1000_TXD_STAT_DD as u8
    }

    /// Free the old buffer in this descriptor and replace it with a new buffer.
    pub fn replace_buffer(&mut self, buf: KernelBuffer) {
        if self.addr != 0 {
            unsafe { kfree(self.addr as *mut c_void) };
        }
        self.addr = buf.page.0 as u64;
        self.length = buf.length as u16;
    }
}

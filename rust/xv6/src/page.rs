//! Types representing a memory page.
use core::{marker::PhantomData, mem::ManuallyDrop};

use crate::bindings;

pub struct Page(pub *mut core::ffi::c_char);
impl Page {
    pub const fn new(ptr: *mut core::ffi::c_char) -> Self {
        Self(ptr)
    }
}
impl Drop for Page {
    fn drop(&mut self) {
        unsafe {
            bindings::kfree(self.0 as _);
        }
    }
}

pub trait BufState {
    const FREE_ON_DROP: bool;
}
/// This marker indicates the buffer is currently owned by the kernel and the underlying page will
/// be freed once it goes out of scope.
pub struct HostOwned;
impl BufState for HostOwned {
    const FREE_ON_DROP: bool = true;
}
/// This marker indicates the buffer is currently owned by a device, the page will not be freed
/// when it goes out of scope.
pub struct DeviceOwned;
impl BufState for DeviceOwned {
    const FREE_ON_DROP: bool = false;
}

pub struct KernelBuffer<S: BufState = HostOwned> {
    page: ManuallyDrop<Page>,
    pub length: usize,
    _s: PhantomData<S>,
}

impl<S: BufState> KernelBuffer<S> {
    pub const fn new(ptr: *mut core::ffi::c_char, len: usize) -> Self {
        Self {
            page: ManuallyDrop::new(Page::new(ptr)),
            length: len,
            _s: PhantomData,
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: page is non-null
        unsafe { core::slice::from_raw_parts(self.page.0 as *const u8, self.length) }
    }

    #[inline]
    fn into_state<T: BufState>(self) -> KernelBuffer<T> {
        let this = ManuallyDrop::new(self);
        // SAFETY: we're moving fields out; `this` won't be dropped
        let page = unsafe { core::ptr::read(&this.page) };

        KernelBuffer {
            page,
            length: this.length,
            _s: PhantomData,
        }
    }
}

impl KernelBuffer<HostOwned> {
    pub fn into_device(self) -> KernelBuffer<DeviceOwned> {
        self.into_state::<DeviceOwned>()
    }
}
impl KernelBuffer<DeviceOwned> {
    pub fn dma_address(&self) -> u64 {
        self.page.0 as u64
    }
}
impl<S: BufState> Drop for KernelBuffer<S> {
    fn drop(&mut self) {
        if S::FREE_ON_DROP {
            unsafe {
                ManuallyDrop::drop(&mut self.page);
            }
        }
    }
}

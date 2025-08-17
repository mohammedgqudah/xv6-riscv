//! A kernel mutex.
//!
//! Uses the spinlock implementation in `spinlock.c`
use crate::bindings::{self};
use core::{
    cell::UnsafeCell,
    ffi::CStr,
    ops::{Deref, DerefMut},
};

pub struct Mutex<T: Sized> {
    spinlock: UnsafeCell<bindings::spinlock>,
    inner: UnsafeCell<T>,
}

unsafe impl<T> Sync for Mutex<T> {}
unsafe impl<T> Send for Mutex<T> {}

impl<T> Mutex<T> {
    pub const fn new(inner: T, name: &'static CStr) -> Self {
        Self {
            spinlock: UnsafeCell::new(bindings::spinlock {
                name: name.as_ptr() as _,
                cpu: core::ptr::null_mut(),
                locked: 0,
            }),
            inner: UnsafeCell::new(inner),
        }
    }

    pub fn lock(&'_ self) -> MutexGuard<'_, T> {
        // SAFETY: `self.spinlock` is a valid initilized structure.
        unsafe {
            bindings::acquire(self.spinlock.get());
        }
        MutexGuard { mutex: self }
    }
}

#[must_use = "dropping the guard releases the lock"]
pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
}

impl<'a, T> Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAETY: we have exclusive access
        unsafe {
            self.mutex
                .inner
                .get()
                .as_mut()
                .expect("inner should not be null")
        }
    }
}

impl<'a, T> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            self.mutex
                .inner
                .get()
                .as_mut()
                .expect("inner should not be null")
        }
    }
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        // SAFETY: `self.spinlock` is a valid initilized structure.
        unsafe {
            bindings::release(self.mutex.spinlock.get());
        }
    }
}

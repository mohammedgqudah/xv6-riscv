//! A kernel mutex.
//!
//! Uses the spinlock implementation in `spinlock.c`
use crate::bindings::{self};
use core::{
    cell::UnsafeCell,
    ffi::CStr,
    mem::ManuallyDrop,
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

impl<'a, T> MutexGuard<'a, T> {
    /// Release the mutex and put the current process to sleep
    /// on the mutex's wait-channel; on wakeup, the mutex is re-acquired
    /// and a fresh guard is returned.
    ///
    /// Note: This takes `self` by value, so no borrows of `T` (e.g., `&mut T`)
    /// can remain live across the call, to preserve Rust's aliasing XOR mutability rule.
    pub fn proc_sleep(self) -> MutexGuard<'a, T> {
        // `sleep` will release the lock, so don't automatically
        // unlock (via the destructor)
        let this = ManuallyDrop::new(self);
        // SAFETY: We are passing a valid pointer to a `spinlock` and
        // the lock is held (by the MutexGuard).
        unsafe {
            bindings::sleep(this.mutex as *const _ as *mut _, this.mutex.spinlock.get());
        };
        // the call to `sleep` returns after wakeup and it re-acquires the lock,
        // so it's safe to construct the MutexGuard (we have exclusive access now).
        MutexGuard { mutex: this.mutex }
    }

    /// Wake sleepers on this mutex's wait-channel. Note that this does
    /// not release the lock, and it's up to the caller to release it.
    pub fn wakeup(&self) {
        unsafe {
            bindings::wakeup(self.mutex as *const _ as *mut _);
        }
    }
}

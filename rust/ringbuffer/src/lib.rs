//! A fixed size ring buffer.
#![cfg_attr(not(test), no_std)]
#![no_builtins]
use core::mem::MaybeUninit;

#[derive(Debug)]
pub enum PushError {
    RingIsFull,
}

#[derive(Debug)]
pub struct RingBuffer<T, const CAPACITY: usize> {
    queue: [MaybeUninit<T>; CAPACITY],
    /// Number of dropped items because the ring was full
    dropped: usize,
    /// reader pointer
    read: usize,
    length: usize,
}

impl<T, const CAPACITY: usize> RingBuffer<T, CAPACITY> {
    pub const fn new() -> Self {
        assert!(
            CAPACITY != 0 && (CAPACITY & (CAPACITY - 1)) == 0,
            "CAPACITY must be a power of 2"
        );

        let buf = [const { MaybeUninit::uninit() }; CAPACITY];
        Self {
            queue: buf,
            dropped: 0,
            read: 0,
            length: 0,
        }
    }

    #[inline(always)]
    pub fn mask(i: usize) -> usize {
        i & (CAPACITY - 1)
    }

    pub fn push(&mut self, item: T) -> Result<(), PushError> {
        if self.is_full() {
            self.dropped += 1;
            return Err(PushError::RingIsFull);
        }

        self.queue[Self::mask(self.read + self.length)].write(item);

        self.length += 1;

        Ok(())
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        let idx = Self::mask(self.read);

        // SAFETY: Every slot in `queue` starts uninitialized
        // but we checked `!self.is_empty()`, which implies
        // that the slot at index `read` was previously initialized
        // by `push` and has not yet been popped.
        let item = unsafe { self.queue[idx].assume_init_read() };

        self.read = Self::mask(self.read + 1);
        self.length -= 1;

        Some(item)
    }

    #[inline(always)]
    pub fn is_full(&self) -> bool {
        self.length == CAPACITY
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Returns the number of elements currently stored in the buffer.
    ///
    /// Not to be confused with `capacity`, which is the
    /// maximum number of elements the buffer can hold.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.length
    }

    #[inline(always)]
    pub const fn capacity(&self) -> usize {
        CAPACITY
    }

    #[inline(always)]
    pub fn dropped_count(&self) -> usize {
        self.dropped
    }

    pub fn clear(&mut self) {
        while self.pop().is_some() {}
        self.read = 0;
    }
}

impl<T, const CAPACITY: usize> Default for RingBuffer<T, CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const CAPACITY: usize> Drop for RingBuffer<T, CAPACITY> {
    fn drop(&mut self) {
        // Ensure we run Drop for any remaining T
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct Item {
        val: i32,
        dropped: bool,
    }
    impl Item {
        pub fn new(val: i32) -> Self {
            Self {
                val,
                dropped: false,
            }
        }
    }

    impl Drop for Item {
        fn drop(&mut self) {
            assert!(
                !self.dropped,
                "Item was dropped twice, something is wrong with the the ring buffer"
            );
            self.dropped = true;
        }
    }

    #[test]
    fn test() {
        let mut ring = RingBuffer::<Item, 4>::new();
        assert!(ring.is_empty());
        assert!(!ring.is_full());
        assert_eq!(ring.len(), 0);

        ring.push(Item::new(1)).expect("ring should not be full");

        assert!(!ring.is_empty());
        assert!(!ring.is_full());
        assert_eq!(ring.len(), 1);

        ring.push(Item::new(2)).expect("ring should not be full");
        ring.push(Item::new(3)).expect("ring should not be full");
        ring.push(Item::new(4)).expect("ring should not be full");

        assert!(!ring.is_empty());
        assert!(ring.is_full());
        assert_eq!(ring.len(), 4);
        assert!(matches!(
            ring.push(Item::new(5)),
            Err(PushError::RingIsFull)
        ));
        assert_eq!(ring.dropped_count(), 1);

        let item = ring.pop().expect("ring should have 4 items");

        assert!(!ring.is_empty());
        assert!(!ring.is_full());
        assert_eq!(ring.len(), 3);
        assert_eq!(item.val, 1);

        assert_eq!(ring.pop().expect("ring should not be empty").val, 2);
        assert_eq!(ring.pop().expect("ring should not be empty").val, 3);
        assert_eq!(ring.pop().expect("ring should not be empty").val, 4);
        assert_eq!(ring.len(), 0);

        ring.push(Item::new(10)).expect("ring should not be full");
        ring.push(Item::new(20)).expect("ring should not be full");
        ring.push(Item::new(30)).expect("ring should not be full");
        ring.push(Item::new(40)).expect("ring should not be full");
    }
}

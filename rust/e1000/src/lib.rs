#![no_std]
#![no_builtins]

mod driver;

use core::ffi::{self};
use xv6::{
    KernelBuffer,
    bindings::{self, RX_RING_SIZE, TX_RING_SIZE, rx_desc, tx_desc},
    mutex::{Mutex, MutexGuard},
};

type TxRingDescriptors = [tx_desc; TX_RING_SIZE as usize];
type RxRingDescriptors = [rx_desc; RX_RING_SIZE as usize];

static TX_RING_LOCK: Mutex<()> = Mutex::new((), c"e1000_tx_ring_lock");
static TX_RING: TxRing = TxRing::new();

static RX_RING_LOCK: Mutex<()> = Mutex::new((), c"e1000_rx_ring_lock");
static RX_RING: RxRing = RxRing::new();

unsafe extern "C" {
    fn get_raw_regs() -> *mut u32;
    fn net_rx(buf: *mut ffi::c_char, len: u32);
}

struct TxRing {}
struct TxRingGuard<'a> {
    ring: &'a mut TxRingDescriptors,
    // guard must be declared after `ring` so that ring is droppped before releasing the lock.
    // see: https://doc.rust-lang.org/reference/destructors.html#r-destructors.operation
    _guard: MutexGuard<'a, ()>,
}

impl TxRing {
    pub const fn new() -> Self {
        Self {}
    }
    pub fn lock(&self) -> TxRingGuard<'_> {
        unsafe extern "C" {
            static mut tx_ring: TxRingDescriptors;
        }

        let guard = TX_RING_LOCK.lock();
        TxRingGuard {
            _guard: guard,
            // SAFETY: We only create this static mutable reference while holding `TX_RING_LOCK`.
            // While the lock guard is alive, *no other pointer is used and no other reference is created.*
            // The reference’s lifetime is tied to the guard via `TxRingGuard<'a>`,
            // and when `TxRingGuard` is dropped, `ring` is dropped so the borrow ends
            // before the lock is released.
            ring: unsafe {
                #[allow(static_mut_refs)]
                &mut tx_ring
            },
        }
    }
}

impl<'a> TxRingGuard<'a> {
    pub fn tail(&mut self) -> &mut tx_desc {
        &mut self.ring[get_register(Registers::TDT)]
    }
}

#[allow(clippy::upper_case_acronyms)]
enum Registers {
    TDT = bindings::E1000_TDT as isize,
    //TDH = bindings::E1000_TDH as isize,
    RDT = bindings::E1000_RDT as isize,
    //RDH = bindings::E1000_RDH as isize,
}

fn get_register(register: Registers) -> usize {
    // SAFETY: `get_raw_regs` returns a valid region in memory and `register` contains a known
    // offset in that region.
    unsafe { core::ptr::read_volatile(get_raw_regs().add(register as usize)) as usize }
}

fn set_register(register: Registers, value: u32) {
    // SAFETY: `get_raw_regs` returns a valid region in memory and `register` contains a known
    // offset in that region.
    unsafe { core::ptr::write_volatile(get_raw_regs().add(register as usize), value) };
}

struct RxRing {}
struct RxRingGuard<'a> {
    ring: &'a mut RxRingDescriptors,
    _guard: MutexGuard<'a, ()>,
}

impl RxRing {
    pub const fn new() -> Self {
        Self {}
    }
    pub fn lock(&self) -> RxRingGuard<'_> {
        unsafe extern "C" {
            static mut rx_ring: RxRingDescriptors;
        }

        let guard = RX_RING_LOCK.lock();
        RxRingGuard {
            _guard: guard,
            // SAFETY: We only create this static mutable reference while holding `RX_RING_LOCK`.
            // While the lock guard is alive, *no other pointer is used and no other reference is created.*
            // The reference’s lifetime is tied to the guard via `RxRingGuard<'a>`,
            // and when `RxRingGuard` is dropped, `ring` is dropped so the borrow ends
            // before the lock is released.
            ring: unsafe {
                #[allow(static_mut_refs)]
                &mut rx_ring
            },
        }
    }
}

impl<'a> RxRingGuard<'a> {}
impl<'a> RxRingGuard<'a> {
    #[inline(always)]
    pub fn desc_mut(&mut self, idx: usize) -> &mut rx_desc {
        &mut self.ring[idx]
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn e1000_transmit(buf: *mut core::ffi::c_char, len: i32) -> i32 {
    match driver::transmit(KernelBuffer::new(buf, len as usize)) {
        Ok(_) => 0,
        Err(_) => 1,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn e1000_recv() {
    driver::receive(|buf_addr: u64, len: u32| unsafe {
        net_rx(buf_addr as *mut ffi::c_char, len);
    });
}

#![no_std]
#![no_builtins]

use core::sync::atomic::{Ordering, fence};

use xv6::{
    KernelBuffer,
    bindings::{self, E1000_TXD_CMD_EOP, E1000_TXD_CMD_RS, TX_RING_SIZE, tx_desc},
    println,
};

unsafe extern "C" {
    safe fn get_raw_regs() -> *mut u32;
    static mut tx_ring: [tx_desc; TX_RING_SIZE as usize];
}

enum Registers {
    TDT = bindings::E1000_TDT as isize,
    //TDH = bindings::E1000_TDH as isize,
    //RDT = bindings::E1000_RDT as isize,
    //RDH = bindings::E1000_RDH as isize,
}

fn get_register(register: Registers) -> usize {
    // SAFETY: `get_raw_regs` returns a value region in memory and `register` contains a known
    // offset in that region.
    unsafe { core::ptr::read_volatile(get_raw_regs().add(register as usize)) as usize }
}

fn set_register(register: Registers, value: u32) {
    // SAFETY: `get_raw_regs` returns a value region in memory and `register` contains a known
    // offset in that region.
    unsafe { core::ptr::write_volatile(get_raw_regs().add(register as usize), value) };
}

// TODO: MutexGuard the ring
fn get_tx_desc(index: usize) -> &'static mut tx_desc {
    assert!(
        index < bindings::TX_RING_SIZE as usize,
        "tx ring out of bounds"
    );

    // SAFETY: the index is within bounds
    unsafe { &mut tx_ring[index] }
}

/// Transmit a buffer.
///
/// This will place `buffer` in the tail of the transmission ring and then signal the NIC of a new
/// packet. Note that this takes ownership of the buffer and the buffer is later freed when this ring
/// slot is "done" and used later, see `replace_buffer`.
fn transmit(buffer: KernelBuffer) -> Result<(), ()> {
    let idx = get_register(Registers::TDT);
    let desc = get_tx_desc(idx);

    if !desc.is_done() {
        println!(
            "[index={}] warning: a previous transaction is already in flight.",
            idx
        );
        return Err(());
    }

    desc.replace_buffer(buffer);
    desc.cmd = E1000_TXD_CMD_RS | E1000_TXD_CMD_EOP;

    // Ensure modifications to the descriptor
    // are globally visible before signaling e1000.
    fence(Ordering::SeqCst);

    set_register(Registers::TDT, ((idx + 1) % TX_RING_SIZE as usize) as u32);

    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn e1000_transmit(buf: *mut core::ffi::c_char, len: i32) -> i32 {
    match transmit(KernelBuffer::new(buf, len as usize)) {
        Ok(_) => 0,
        Err(_) => 1,
    }
}

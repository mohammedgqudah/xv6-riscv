//! The E1000 driver logic.
//! Unsafe code is not allowed here, only safe APIs defined in lib.rs can be uesd.

#[forbid(unsafe_code)]

use core::sync::atomic::{Ordering, fence};
use xv6::{
    KernelBuffer,
    bindings::{E1000_TXD_CMD_EOP, E1000_TXD_CMD_RS, TX_RING_SIZE},
    println,
};
use crate::{Registers, TX_RING, get_register, set_register};

/// Transmit a buffer.
///
/// This will place `buffer` in the tail of the transmission ring and then signal the NIC of a new
/// packet. Note that this takes ownership of the buffer and the buffer is later freed when this ring
/// slot is "done" and used later, see `replace_buffer`.
pub(crate) fn transmit(buffer: KernelBuffer) -> Result<(), ()> {
    let idx = get_register(Registers::TDT);
    let mut ring = TX_RING.lock();
    let desc = ring.tail();

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

    // Advance the ring tail descriptor pointer.
    // This will inform hardware to take the packet we filled above and transmit it.
    set_register(Registers::TDT, ((idx + 1) % TX_RING_SIZE as usize) as u32);

    Ok(())
}

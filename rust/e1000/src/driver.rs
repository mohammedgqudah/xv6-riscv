//! The E1000 driver logic.
//! Unsafe code is not allowed here, only safe APIs defined in lib.rs can be uesd.

use crate::{RX_RING, Registers, TX_RING, get_register, set_register};
use core::sync::atomic::{Ordering, fence};
use xv6::{
    KernelBuffer,
    bindings::{
        E1000_RXD_STAT_EOP, E1000_TXD_CMD_EOP, E1000_TXD_CMD_RS, RX_RING_SIZE, TX_RING_SIZE, kalloc,
    },
    println,
};

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

    desc.replace_buffer(buffer.into_device());
    desc.cmd = E1000_TXD_CMD_RS | E1000_TXD_CMD_EOP;

    // Ensure modifications to the descriptor
    // are globally visible before signaling e1000.
    fence(Ordering::SeqCst);

    // Advance the ring tail descriptor pointer.
    // This will inform hardware to take the packet we filled above and transmit it.
    set_register(Registers::TDT, ((idx + 1) % TX_RING_SIZE as usize) as u32);

    Ok(())
}

/// Recieve all available packets in the rx ring and call `callback` with each.
pub(crate) fn receive<F>(callback: F)
where
    F: Fn(u64, u32),
{
    // TODO: 3.2.3 Software must read multiple descriptors to determine the complete
    // length for packets that span multiple receive buffers
    let mut ring = RX_RING.lock();
    let mut count = 0;

    // Multiple packets could be pending, loop until no more descriptors are done.
    for _ in 0..RX_RING_SIZE {
        let idx = (get_register(Registers::RDT) + 1) % RX_RING_SIZE as usize;
        let desc = ring.desc_mut(idx);

        if !desc.is_done() {
            // descriptor not done, stop scanning.
            break;
        }
        if (desc.status & E1000_RXD_STAT_EOP) == 0 {
            panic!("multi-buffer packets are not supported yet!");
        }

        count += 1;
        callback(desc.addr, desc.length as u32);
        desc.status = 0;
        desc.addr = unsafe { kalloc() as u64 };

        // Ensure modifications to the descriptor
        // are globally visible before signaling e1000.
        fence(Ordering::SeqCst);
        // We have processed this packet, transfer descriptor ownership back to hardware.
        set_register(Registers::RDT, idx as u32);
    }

    println!("*** e1000_recv: processed {} packets", count);
}

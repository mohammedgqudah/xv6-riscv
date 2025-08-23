#![no_std]
#![forbid(clippy::undocumented_unsafe_blocks)]
#![forbid(clippy::missing_safety_doc)]

use core::{ffi, fmt::Debug, mem::MaybeUninit, panic};
use ringbuffer::RingBuffer;
use xv6::{KernelBuffer, bindings, mutex::Mutex, println};

struct Packet {
    pub source_port: u16,
    buffer: KernelBuffer,
}

impl Debug for Packet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("Packet<source: {}>", self.source_port))?;
        Ok(())
    }
}

struct Queue {
    pub ring: RingBuffer<Packet, SOCK_QUEUE_SIZE>,
    port: u16,
    used: bool,
}

impl Queue {
    pub const fn new() -> Self {
        Self {
            port: 0,
            ring: RingBuffer::new(),
            used: false,
        }
    }
}

const SOCK_QUEUE_SIZE: usize = 16;
static QUEUES: [Mutex<Queue>; 10] = [const { Mutex::new(Queue::new(), c"eah") }; 10];

fn alloc_queue(dest_port: u16) -> &'static Mutex<Queue> {
    for q in &QUEUES {
        let mut lq = q.lock();
        if !lq.used {
            lq.port = dest_port;
            lq.used = true;
            return q;
        }
    }

    panic!("all queues are in use!");
}

fn find_queue(dest_port: u16) -> Option<&'static Mutex<Queue>> {
    for q in &QUEUES {
        let lq = q.lock();
        if lq.port == dest_port {
            return Some(q);
        }
    }

    None
}

pub enum IpRecieveError {
    InvalidPacket,
}

pub struct PacketView<'a> {
    inner: &'a [u8],
}

/// SAFETY: TODO: This is probably not safe, rust references must be aligned, so I have two options:
/// 1. return a copy using ptr::read_unaligned (I dont want copies)
/// 2. look into the zerocopy copy and how unaligned views are implemented.
impl<'a> PacketView<'a> {
    pub fn eth(&self) -> &'a bindings::eth {
        assert!(
            self.inner.len() >= size_of::<bindings::eth>(),
            "packet is not large enough"
        );
        // SAFETY: This offset is a valid pointer within `self.inner` and is initlized
        unsafe {
            (self.inner.as_ptr() as *const bindings::eth)
                .as_ref()
                .unwrap()
        }
    }
    pub fn ip(&self) -> &'a bindings::ip {
        assert!(
            self.inner.len() >= size_of::<bindings::eth>() + size_of::<bindings::ip>(),
            "packet is not large enough"
        );
        // SAFETY: This offset is a valid pointer within `self.inner` and is initlized
        unsafe {
            (self.inner.as_ptr().add(size_of::<bindings::eth>()) as *const bindings::ip)
                .as_ref()
                .unwrap()
        }
    }

    pub fn udp(&self) -> &'a bindings::udp {
        assert!(
            self.inner.len()
                >= size_of::<bindings::eth>()
                    + size_of::<bindings::ip>()
                    + size_of::<bindings::udp>(),
            "packet is not large enough"
        );
        // SAFETY: This offset is a valid pointer within `self.inner` and is initlized
        // SAFETY: TODO: This assumes the pointer is aligned and ip header is fixed.
        unsafe {
            (self
                .inner
                .as_ptr()
                .add(size_of::<bindings::eth>())
                .add(size_of::<bindings::ip>()) as *const bindings::udp)
                .as_ref()
                .unwrap()
        }
    }
    pub fn payload_ptr(&self) -> *const u8 {
        // SAFETY: This offset is a valid pointer within `self.inner`
        unsafe {
            self.inner
                .as_ptr()
                .add(size_of::<bindings::eth>())
                .add(size_of::<bindings::ip>())
                .add(size_of::<bindings::udp>())
        }
    }

    pub fn udp_len(&self) -> i32 {
        u16::from_be(self.udp().ulen) as i32 - core::mem::size_of::<bindings::udp>() as i32
    }

    pub fn from_kernel_buffer(buf: &'a KernelBuffer) -> Self {
        Self {
            inner: buf.as_slice(),
        }
    }
}

fn ip_receive(buffer: KernelBuffer) -> Result<(), IpRecieveError> {
    // SAFETY: don't delete this print; make grade depends on it.
    unsafe {
        static mut IP_SEEN: bool = false;
        if !IP_SEEN {
            println!("ip_rx: received an IP packet");
            IP_SEEN = true;
        }
    }

    let pv = PacketView::from_kernel_buffer(&buffer);
    let udp = pv.udp();
    let Some(mut _queue) = find_queue(u16::from_be(udp.dport)) else {
        return Ok(());
    };
    let mut queue = _queue.lock();

    let packet = Packet {
        source_port: u16::from_be(udp.sport),
        buffer,
    };
    match queue.ring.push(packet) {
        Err(ringbuffer::PushError::RingIsFull) => (),
        Ok(_) => {
            queue.wakeup();
        }
    }

    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn ip_rx(buf: *mut ffi::c_char, len: u32) {
    let _ = ip_receive(KernelBuffer::new(buf, len as usize));
}

#[unsafe(no_mangle)]
/// A system call to recv a packet payload.
///
/// If the packet queue for `dport` is empty, then the process will sleep until more packets
/// are queued.
pub extern "C" fn sys_recv_impl(
    dport: u16,
    srcaddr: u64,
    sportaddr: u64,
    bufaddr: u64,
    maxlen: u32,
) -> i32 {
    let mut maxlen = maxlen;
    let mut _queue = find_queue(dport);
    let Some(_queue) = _queue else {
        panic!("recv called but no queue was allocated")
    };

    let mut queue = _queue.lock();

    // sleep until the queue is not empty
    loop {
        if !queue.ring.is_empty() {
            break;
        }
        queue = queue.proc_sleep();
    }

    let packet = match queue.ring.pop() {
        Some(pkt) => pkt,
        None => return -1,
    };

    let pv = PacketView::from_kernel_buffer(&packet.buffer);
    let ipsrc = u32::from_be(pv.ip().ip_src);

    if maxlen as i32 > pv.udp_len() {
        maxlen = pv.udp_len() as u32;
    }

    // SAFETY: we are passing valid `src` addresses and `copyout` will check if the user
    // address is a valid address in the process address space.
    unsafe {
        let pagetable = bindings::myproc().as_ref().unwrap().pagetable;
        bindings::copyout(
            pagetable,
            sportaddr,
            (&packet.source_port) as *const u16 as *mut _,
            core::mem::size_of::<u16>() as u64,
        );
        bindings::copyout(
            pagetable,
            srcaddr,
            &ipsrc as *const u32 as *mut _,
            core::mem::size_of::<u32>() as u64,
        );
        bindings::copyout(
            pagetable,
            bufaddr,
            pv.payload_ptr() as *mut _,
            maxlen as u64,
        );
    }

    maxlen as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn sys_bind() -> i32 {
    let mut port = MaybeUninit::<i32>::uninit();
    // SAFETY: a valid mut ptr is passed to argint, which is then initialized
    let port = unsafe {
        bindings::argint(0, port.as_mut_ptr());
        port.assume_init()
    };
    alloc_queue(port as u16);
    println!("sys_bind: {}", port);
    1
}

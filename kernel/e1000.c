#include "types.h"
#include "param.h"
#include "memlayout.h"
#include "riscv.h"
#include "spinlock.h"
#include "proc.h"
#include "defs.h"
#include "e1000_dev.h"

extern struct tx_desc tx_ring[TX_RING_SIZE];
struct tx_desc tx_ring[TX_RING_SIZE] __attribute__((aligned(16)));

static char *tx_bufs[TX_RING_SIZE];

#define RX_RING_SIZE 16
static struct rx_desc rx_ring[RX_RING_SIZE] __attribute__((aligned(16)));
static char *rx_bufs[RX_RING_SIZE];

// remember where the e1000's registers live.
volatile uint32 *regs;

struct spinlock e1000_lock_tx;
struct spinlock e1000_lock_rx;

volatile uint32 *get_raw_regs() {
  return regs;
}

// called by pci_init().
// xregs is the memory address at which the
// e1000's registers are mapped.
void
e1000_init(uint32 *xregs)
{
  int i;

  initlock(&e1000_lock_tx, "e1000 tx lock");
  initlock(&e1000_lock_rx, "e1000 rx lock");

  regs = xregs;

  // Reset the device
  regs[E1000_IMS] = 0; // disable interrupts
  regs[E1000_CTL] |= E1000_CTL_RST;
  regs[E1000_IMS] = 0; // redisable interrupts
  __sync_synchronize();

  // [E1000 14.5] Transmit initialization
  memset(tx_ring, 0, sizeof(tx_ring));
  for (i = 0; i < TX_RING_SIZE; i++) {
    tx_ring[i].status = E1000_TXD_STAT_DD;
    tx_bufs[i] = 0;
  }
  regs[E1000_TDBAL] = (uint64) tx_ring;
  if(sizeof(tx_ring) % 128 != 0)
    panic("e1000");
  regs[E1000_TDLEN] = sizeof(tx_ring);
  regs[E1000_TDH] = regs[E1000_TDT] = 0;
  
  // [E1000 14.4] Receive initialization
  memset(rx_ring, 0, sizeof(rx_ring));
  for (i = 0; i < RX_RING_SIZE; i++) {
    rx_bufs[i] = kalloc();
    if (!rx_bufs[i])
      panic("e1000");
    rx_ring[i].addr = (uint64) rx_bufs[i];
  }
  regs[E1000_RDBAL] = (uint64) rx_ring;
  if(sizeof(rx_ring) % 128 != 0)
    panic("e1000");
  regs[E1000_RDH] = 0;
  regs[E1000_RDT] = RX_RING_SIZE - 1;
  regs[E1000_RDLEN] = sizeof(rx_ring);

  // filter by qemu's MAC address, 52:54:00:12:34:56
  // 3.2.1 packet filtering.
  regs[E1000_RA] = 0x12005452; // low
  regs[E1000_RA+1] = 0x5634 | (1<<31); // high
  // multicast table
  for (int i = 0; i < 4096/32; i++)
    regs[E1000_MTA + i] = 0;

  // transmitter control bits.
  regs[E1000_TCTL] = E1000_TCTL_EN |  // enable
    E1000_TCTL_PSP |                  // pad short packets
    (0x10 << E1000_TCTL_CT_SHIFT) |   // collision stuff
    (0x40 << E1000_TCTL_COLD_SHIFT);
  regs[E1000_TIPG] = 10 | (8<<10) | (6<<20); // inter-pkt gap

  // receiver control bits.
  regs[E1000_RCTL] = E1000_RCTL_EN | // enable receiver
    E1000_RCTL_BAM |                 // enable broadcast
    E1000_RCTL_SZ_2048 |             // 2048-byte rx buffers
    E1000_RCTL_SECRC;                // strip CRC
  
  // ask e1000 for receive interrupts.
  regs[E1000_RDTR] = 0; // interrupt after every received packet (no timer)
  regs[E1000_RADV] = 0; // interrupt after every packet (no timer)
  regs[E1000_IMS] = (1 << 7); // RXDW -- Receiver Descriptor Write Back
}

int e1000_transmit(char *buf, int len);

static void
e1000_recv(void)
{
  // TODO: 3.2.3 Software must read multiple descriptors to determine the complete
  // length for packets that span multiple receive buffers
  
  // loop because multiple packets could be ready and not just one.
  acquire(&e1000_lock_rx);

  int i;
  for (i = 0; i < RX_RING_SIZE; ++i) {
    int idx = (regs[E1000_RDT] + 1) % RX_RING_SIZE;
    struct rx_desc *desc = &rx_ring[idx];
    if ((desc->status & E1000_RXD_STAT_DD) != E1000_RXD_STAT_DD) {
      // if the descriptor is not done, stop.
      // (we have reached the head,  I think? so it's not possible for descriptors after this to be ready)
      // Note: I imagine this happens because we always call e1000 on interrupts,
      // and maybe the signaled interrupt is not for ready packets.
      break;
    }
    
    if ((desc->status & E1000_RXD_STAT_EOP) == 0) {
      panic("multi-buffer packets are not supported yet.");
      break;
    }

    //printf("* e1000_recv: processing descriptor[%d]\n", idx);
    net_rx((char*)desc->addr, desc->length);
  
    // not sure if i need to update rx_buf array for this index or not. not sure why we need rx_buf at all.
    desc->addr = (uint64)kalloc();
    desc->status = 0;
    __sync_synchronize();

    // we have processed this packet, increment the tail to transfer ownership of the descriptor back to the hardware.
    regs[E1000_RDT] = idx;
  }
  
  release(&e1000_lock_rx);
  if (i > 0)
    printf("*** e1000_recv: processed %d packets\n", i);
}

void
e1000_intr(void)
{
  // tell the e1000 we've seen this interrupt;
  // without this the e1000 won't raise any
  // further interrupts.
  regs[E1000_ICR] = 0xffffffff;

  //printf("e1000 interrupt\n");

  e1000_recv();
}

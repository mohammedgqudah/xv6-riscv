#include "types.h"
#include "param.h"
#include "memlayout.h"
#include "riscv.h"
#include "spinlock.h"
#include "proc.h"
#include "defs.h"
#include "fs.h"
#include "sleeplock.h"
#include "file.h"
#include "net.h"

// xv6's ethernet and IP addresses
static uint8 local_mac[ETHADDR_LEN] = { 0x52, 0x54, 0x00, 0x12, 0x34, 0x56 };
static uint32 local_ip = MAKE_IP_ADDR(10, 0, 2, 15);

// qemu host's ethernet address.
static uint8 host_mac[ETHADDR_LEN] = { 0x52, 0x55, 0x0a, 0x00, 0x02, 0x02 };

static struct spinlock netlock;

void
netinit(void)
{
  initlock(&netlock, "netlock");
}

// must be power of 2, for the uint32 overflow in the ring buffer trick to work properly.
#define MAX_QUEUE_LEN 16

struct packet {
  char *buf;
  int sport;
  int len;
};

// a ring buffer used to queue pakcets for a process bound to a port.
// when the ring buffer is full, additional packets are dropped.
// inspiration: https://www.snellman.net/blog/archive/2016-12-13-ring-buffers/
struct bind_ring_buffer {
  struct spinlock lock;
  struct packet queue[MAX_QUEUE_LEN];
  uint32 read;
  uint32 write;
  int dropped;
  int dport;
};
void ring_init(struct bind_ring_buffer *ring) {
  memset(ring->queue, 0, sizeof(struct packet) * MAX_QUEUE_LEN);
  ring->read = 0;
  ring->write = 0;
  ring->dropped = 0;
  initlock(&ring->lock, "ring");
}
int ring_mod(int num) { return num % MAX_QUEUE_LEN; }
int ring_empty(struct bind_ring_buffer *ring) { return ring->write == ring->read; }
// return the ring buffer size; the number of packets that haven't been consumed yet.
int ring_size(struct bind_ring_buffer *ring) { return ring->write - ring->read; }
int ring_full(struct bind_ring_buffer *ring) { return ring_size(ring) == MAX_QUEUE_LEN; }
int ring_enqueue(struct bind_ring_buffer *ring, struct packet packet) {
  if (ring_full(ring)) {
    ring->dropped += 1;
    return 1;
  }
  ring->queue[ring_mod(ring->write++)] = packet;
  return 0;
}
int ring_dequeue(struct bind_ring_buffer *ring, struct packet *packet) {
  if (ring_empty(ring)) {
    return 1;
  }
  *packet = ring->queue[ring_mod(ring->read++)];
  return 0;
}


// store a fixed size of ring buffers for now, use a btree later or a hashamp;
#define RINGS_NUM 100
static struct bind_ring_buffer rings[RINGS_NUM] = {0}; 

// find the next unsed ring buffer and update
// its dport to the port requested.
//
// # Return value
// This will always return a pointer to a ring buffer or panic if no ring buffer is free.
struct bind_ring_buffer *next_free_ring(int port) {
  for (int i =0; i < RINGS_NUM; i++) {
    if (rings[i].dport == 0) {
      rings[i].dport = port;
      return &rings[i]; 
    }
  }

  panic("all ring buffers are used, time for a serious implementation?");

  return 0;
}

struct bind_ring_buffer *find_ring(int port) {
  for (int i =0; i < RINGS_NUM; i++) {
    if (rings[i].dport == port) {
      return &rings[i]; 
    }
  }

  return 0;
}

//
// bind(int port)
// prepare to receive UDP packets address to the port,
// i.e. allocate any queues &c needed.
//
uint64
sys_bind(void);

//
// unbind(int port)
// release any resources previously created by bind(port);
// from now on UDP packets addressed to port should be dropped.
//
uint64
sys_unbind(void)
{
  //
  // Optional: Your code here.
  //

  return 0;
}

uint64 sys_recv_impl(uint16 dport, uint64 srcaddr, uint64 sportaddr, uint64 bufaddr, int maxlen);
//
// recv(int dport, int *src, short *sport, char *buf, int maxlen)
// if there's a received UDP packet already queued that was
// addressed to dport, then return it.
// otherwise wait for such a packet.
//
// sets *src to the IP source address.
// sets *sport to the UDP source port.
// copies up to maxlen bytes of UDP payload to buf.
// returns the number of bytes copied,
// and -1 if there was an error.
//
// dport, *src, and *sport are host byte order.
// bind(dport) must previously have been called.
//
uint64
sys_recv(void)
{
  int dport;
  uint64 srcaddr;
  uint64 sportddr;
  uint64 bufaddr;
  int maxlen;
  
  argint(0, &dport);
  argaddr(1, &srcaddr);
  argaddr(2, &sportddr);
  argaddr(3, &bufaddr);
  argint(4, &maxlen);

  return sys_recv_impl(dport, srcaddr, sportddr, bufaddr, maxlen);
}

// This code is lifted from FreeBSD's ping.c, and is copyright by the Regents
// of the University of California.
static unsigned short
in_cksum(const unsigned char *addr, int len)
{
  int nleft = len;
  const unsigned short *w = (const unsigned short *)addr;
  unsigned int sum = 0;
  unsigned short answer = 0;

  /*
   * Our algorithm is simple, using a 32 bit accumulator (sum), we add
   * sequential 16 bit words to it, and at the end, fold back all the
   * carry bits from the top 16 bits into the lower 16 bits.
   */
  while (nleft > 1)  {
    sum += *w++;
    nleft -= 2;
  }

  /* mop up an odd byte, if necessary */
  if (nleft == 1) {
    *(unsigned char *)(&answer) = *(const unsigned char *)w;
    sum += answer;
  }

  /* add back carry outs from top 16 bits to low 16 bits */
  sum = (sum & 0xffff) + (sum >> 16);
  sum += (sum >> 16);
  /* guaranteed now that the lower 16 bits of sum are correct */

  answer = ~sum; /* truncate to 16 bits */
  return answer;
}

//
// send(int sport, int dst, int dport, char *buf, int len)
//
uint64
sys_send(void)
{
  struct proc *p = myproc();
  int sport;
  int dst;
  int dport;
  uint64 bufaddr;
  int len;

  argint(0, &sport);
  argint(1, &dst);
  argint(2, &dport);
  argaddr(3, &bufaddr);
  argint(4, &len);

  int total = len + sizeof(struct eth) + sizeof(struct ip) + sizeof(struct udp);
  if(total > PGSIZE)
    return -1;

  char *buf = kalloc();
  if(buf == 0){
    printf("sys_send: kalloc failed\n");
    return -1;
  }
  memset(buf, 0, PGSIZE);

  struct eth *eth = (struct eth *) buf;
  memmove(eth->dhost, host_mac, ETHADDR_LEN);
  memmove(eth->shost, local_mac, ETHADDR_LEN);
  eth->type = htons(ETHTYPE_IP);

  struct ip *ip = (struct ip *)(eth + 1);
  ip->ip_vhl = 0x45; // version 4, header length 4*5
  ip->ip_tos = 0;
  ip->ip_len = htons(sizeof(struct ip) + sizeof(struct udp) + len);
  ip->ip_id = 0;
  ip->ip_off = 0;
  ip->ip_ttl = 100;
  ip->ip_p = IPPROTO_UDP;
  ip->ip_src = htonl(local_ip);
  ip->ip_dst = htonl(dst);
  ip->ip_sum = in_cksum((unsigned char *)ip, sizeof(*ip));

  struct udp *udp = (struct udp *)(ip + 1);
  udp->sport = htons(sport);
  udp->dport = htons(dport);
  udp->ulen = htons(len + sizeof(struct udp));

  char *payload = (char *)(udp + 1);
  if(copyin(p->pagetable, payload, bufaddr, len) < 0){
    kfree(buf);
    printf("send: copyin failed\n");
    return -1;
  }

  e1000_transmit(buf, total);

  return 0;
}

void ip_rx(char *buf, int len);

//
// send an ARP reply packet to tell qemu to map
// xv6's ip address to its ethernet address.
// this is the bare minimum needed to persuade
// qemu to send IP packets to xv6; the real ARP
// protocol is more complex.
//
void
arp_rx(char *inbuf)
{
  static int seen_arp = 0;

  if(seen_arp){
    kfree(inbuf);
    return;
  }
  printf("arp_rx: received an ARP packet\n");
  seen_arp = 1;

  struct eth *ineth = (struct eth *) inbuf;
  struct arp *inarp = (struct arp *) (ineth + 1);

  char *buf = kalloc();
  if(buf == 0)
    panic("send_arp_reply");
  
  struct eth *eth = (struct eth *) buf;
  memmove(eth->dhost, ineth->shost, ETHADDR_LEN); // ethernet destination = query source
  memmove(eth->shost, local_mac, ETHADDR_LEN); // ethernet source = xv6's ethernet address
  eth->type = htons(ETHTYPE_ARP);

  struct arp *arp = (struct arp *)(eth + 1);
  arp->hrd = htons(ARP_HRD_ETHER);
  arp->pro = htons(ETHTYPE_IP);
  arp->hln = ETHADDR_LEN;
  arp->pln = sizeof(uint32);
  arp->op = htons(ARP_OP_REPLY);

  memmove(arp->sha, local_mac, ETHADDR_LEN);
  arp->sip = htonl(local_ip);
  memmove(arp->tha, ineth->shost, ETHADDR_LEN);
  arp->tip = inarp->sip;

  e1000_transmit(buf, sizeof(*eth) + sizeof(*arp));

  kfree(inbuf);
}

void
net_rx(char *buf, int len)
{
  struct eth *eth = (struct eth *) buf;

  if(len >= sizeof(struct eth) + sizeof(struct arp) &&
     ntohs(eth->type) == ETHTYPE_ARP){
    arp_rx(buf);
  } else if(len >= sizeof(struct eth) + sizeof(struct ip) &&
     ntohs(eth->type) == ETHTYPE_IP){
    ip_rx(buf, len);
  } else {
    kfree(buf);
  }
}

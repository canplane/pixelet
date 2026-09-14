// script/vector/xpute/core/ds/buddy.ts
//
// A seeded sequence of acquires and releases over a backed pool — spills,
// plain regions past the backing, evictions — and the offset, handle and
// stat after each, for crates/xpute/core/src/ds/buddy.test.rs.

import { type Block, BuddyAllocator } from "@xpute/core/ds/buddy.ts";
import { seeded } from "../../../../vector.ts";

export function vectors() {
  const rnd = seeded(0xb0dd);
  const backing = new ArrayBuffer(1 << 18);
  const pool = new BuddyAllocator({ min_block_bytes: 1 << 8, region_bytes: 1 << 16, max_idle_regions: 1, backing });
  const live: Block[] = [];
  const steps = [];
  for (let k = 0; k < 400; k++) {
    if (live.length > 0 && rnd() % 5 < 2) {
      const b = live.splice(rnd() % live.length, 1)[0];
      pool.release(b);
      steps.push({
        op: "release",
        at: b.byte_offset,
        backed: b.buf === backing,
        bytes: b.bytes,
        live_bytes: pool.stat.live_bytes,
        regions: pool.stat.regions,
        merges: pool.stat.merges,
        evictions: pool.stat.evictions,
      });
    } else {
      const bytes = 1 + (rnd() % (1 << ((rnd() % 17) + 1)));
      const b = pool.acquire(bytes);
      live.push(b);
      const handle = b.order >= 0 ? pool.handle_at(b.buf, b.byte_offset, bytes) : -1;
      steps.push({
        op: "acquire",
        bytes,
        at: b.byte_offset,
        backed: b.buf === backing,
        order: b.order,
        region: b.region,
        handle,
        live_bytes: pool.stat.live_bytes,
        regions: pool.stat.regions,
        splits: pool.stat.splits,
        spills: pool.stat.spills,
      });
    }
  }
  return steps;
}

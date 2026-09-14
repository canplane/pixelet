// script/vector/pxl/shared/spec/space/indexer.ts
//
// Every level helper over addresses on both sides of every seam, and
// SPAN_CENTER at every shift, for crates/pxl/shared/src/spec/space/indexer.test.rs.

import * as ix from "@pxl/shared/spec/space/indexer.ts";
import { ivec, type PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { seeded } from "../../../../../vector.ts";

export function vectors() {
  const rnd = seeded(0x1d3e);
  const addrs: bigint[] = [];
  for (const x of [-4097n, -4096n, -1025n, -1024n, -65n, -64n, -1n, 0n, 1n, 63n, 64n, 1023n, 1024n, 4095n, 4096n, (1n << 23n) - 1n, -(1n << 23n)]) {
    addrs.push(ivec.gather(x, -x - 1n, x % 30000n));
  }
  for (let k = 0; k < 24; k++) addrs.push(ivec.gather(BigInt((rnd() | 0) >> 8), BigInt((rnd() | 0) >> 8), BigInt((rnd() | 0) >> 16)));
  const fns = [
    "L2_BASE",
    "L3_BASE",
    "L4_BASE",
    "L2_CENTER",
    "L3_CENTER",
    "L4_CENTER",
    "L2_LIMIT",
    "L3_LIMIT",
    "L4_LIMIT",
    "L1_IDX_IN_L2",
    "L1_IDX_IN_L3",
    "L2_IDX_IN_L3",
    "L1_IDX_IN_L4",
    "L2_IDX_IN_L4",
    "L3_IDX_IN_L4",
    "L2_IDX",
    "L3_IDX",
    "L4_IDX",
    "FROM_L2_IDX",
    "FROM_L3_IDX",
    "FROM_L4_IDX",
  ] as const;
  return {
    constants: {
      L2_RANGE: ix.L2_RANGE,
      L3_RANGE: ix.L3_RANGE,
      L4_RANGE: ix.L4_RANGE,
      L2_SPAN_IN_L3: ix.L2_SPAN_IN_L3,
      L2_SPAN_IN_L4: ix.L2_SPAN_IN_L4,
      L3_SPAN_IN_L4: ix.L3_SPAN_IN_L4,
      L2_MASK: ix.L2_MASK,
      L3_MASK: ix.L3_MASK,
      L4_MASK: ix.L4_MASK,
    },
    addrs: addrs.map((a) => ({
      a,
      ...Object.fromEntries(fns.map((n) => [n, (ix[n] as (v: bigint) => bigint)(a)])),
      span: [0, 1, 3, 6, 10, 12].map((shamt) => {
        const c: PVF3 = ix.SPAN_CENTER({ i: 0n, f: 0n }, a, shamt);
        return { i: c.i, f: c.f };
      }),
    })),
  };
}

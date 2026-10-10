// script/vector/pxl/shared/spec/space/sparse_tree/tree.ts
//
// A sequence of writes at every level into trees of both dims, then what
// read, walk, compact and encode give — for
// crates/pxl/shared/src/spec/space/sparse_tree/tree.test.rs.

import { type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import * as sparse_tree from "@pxl/shared/spec/space/sparse_tree/tree.ts";
import { bytes_to_hex } from "@xpute/core/kit/encoding.ts";
import { seeded } from "../../../../../../vector.ts";

export function vectors() {
  const rnd = seeded(0x7ee);
  const at = (x: number, y: number, z: number): PVF3 => pvf3.gather({ i: 0n, f: 0n }, BigInt(x), BigInt(y), BigInt(z));
  return [{ dim: 3, min: -3, max: 3 }, { dim: 2, min: -2, max: 4 }].map((layout) => {
    const tree = sparse_tree.alloc(layout as sparse_tree.SparseTreeLayout, 8, Uint16Array);
    const span = 1 << (layout.max - layout.min);
    const unit = 256 >> -layout.min;
    const writes = [];
    for (let k = 0; k < 60; k++) {
      const x = (rnd() % span) * unit, y = (rnd() % span) * unit, z = layout.dim === 3 ? (rnd() % span) * unit : 0;
      const level = layout.min + (rnd() % (layout.max - layout.min));
      const val = rnd() % 5;
      sparse_tree.write(tree, at(x, y, z), val, level);
      writes.push({ x, y, z, level, val });
    }
    const probes = [];
    for (let k = 0; k < 40; k++) {
      const x = (rnd() % span) * unit, y = (rnd() % span) * unit, z = layout.dim === 3 ? (rnd() % span) * unit : 0;
      probes.push({ x, y, z, val: sparse_tree.read(tree, at(x, y, z)) });
    }
    const walked: number[][] = [];
    sparse_tree.walk(tree, (x, y, z, level, val) => walked.push([x, y, z, level, val]));
    const head = tree.mem.head;
    sparse_tree.compact(tree);
    const blob = sparse_tree.encode(tree);
    return {
      dim: layout.dim,
      min: layout.min,
      max: layout.max,
      writes,
      probes,
      walked: walked.map((w) => w.join(",")),
      head,
      compacted_head: tree.mem.head,
      encoded: bytes_to_hex(new Uint8Array(blob.buffer, blob.byteOffset, blob.byteLength)),
    };
  });
}

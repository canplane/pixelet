// script/vector/pxl/shared/record/table.ts
//
// A record with every column shape — number and bigint lanes, a strided
// lane, strings — as the TypeScript writes it, for
// crates/pxl/shared/src/record/table.test.rs.

import { encode_table, Table } from "@pxl/shared/record/table.ts";
import { bytes_to_hex } from "@xpute/core/kit/encoding.ts";

export function vectors() {
  const pkt = encode_table("thing", 7, 3, [
    { key: "a", value: new Uint32Array([1, 2, 0xffffffff]), required: true },
    { key: "b", value: new BigInt64Array([-1n, 0n, 1n << 62n]) },
    { key: "v", value: new Float32Array([0.5, 1, 1.5, 2, 2.5, 3]), stride: 2, required: true },
    { key: "s", value: ["", "가", "x"] },
  ]);
  const t = new Table(pkt, ["a", "b", "v", "s"]);
  return { packet: bytes_to_hex(pkt), kind: t.kind, version: t.version, count: t.count, keys: t.keys };
}

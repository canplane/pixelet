// script/vector/xpute/core/ipc/frame.ts
//
// The tx_id packing, align8 across u32 and a frame's layout — a body that
// fits, one that spills, and args past the frame — for
// crates/xpute/core/src/ipc/frame.test.rs.

import { align8, Frame, make_tx_id, tx_seq, tx_worker_id } from "@xpute/core/ipc/frame.ts";

export function vectors() {
  const lay_out = (bytes: number, args: number, body: number) => {
    const f = new Frame(new ArrayBuffer(bytes));
    f.begin(make_tx_id(3, 77), 0x1234, 2);
    let at: number | string;
    try {
      at = f.lay_out(args, body);
    } catch {
      at = "throws";
    }
    return { bytes, args, body, at, words: Array.from(new Uint32Array(f.buf, 0, 8)) };
  };
  return {
    tx: [[0, 0], [3, 77], [255, 0xffffff], [256, 1]].map(([w, s]) => {
      const id = make_tx_id(w, s);
      return { w, s, id, worker: tx_worker_id(id), seq: tx_seq(id) };
    }),
    align8: [0, 1, 7, 8, 9, 0x7ffffff9, 0xfffffff8, 0xfffffff9, 0xffffffff].map((n) => ({ n, a: align8(n) })),
    lay_out: [lay_out(64, 4, 16), lay_out(64, 4, 64), lay_out(64, 64, 0), lay_out(64, 0xffffffe0, 0)],
  };
}

// @xpute/host/stop.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { Golden } from "@xpute/golden";
import { td } from "@xpute/kit/codec/encoding.ts";
import { ERRNO_EIO } from "@xpute/kit/status/errno.spec.ts";
import { Lift, Lower } from "@xpute/link/cabi.ts";
import { type LinkObject, pkt_of, type Side } from "@xpute/link/link.ts";
import { HANDLE, STOPPED } from "./abi/stop/stop.ts";
import { init, make_stop, type Stop } from "./runtime.ts";
import { mount_stop } from "./stop.ts";

test("a stop that came across the link, and one made of a throw's message, hold a file and a message past the cap cut as the record of the cut holds them", async () => {
  const g = await Golden.load("kit/spec/golden/status/msg.tsv");
  const [long, cut] = ["straddle.in", "straddle.out"].map((k) =>
    td.decode(g.bytes(k)),
  ) as [string, string];
  const made = make_stop(ERRNO_EIO, long, 7, 1n, 2n, long);
  assert.deepEqual([made.file, made.msg], [cut, cut]);
  assert.equal(make_stop(ERRNO_EIO, "a/b.ts", 7, 1n, 2n, "").msg, null);

  const stops: Stop[] = [];
  init({ stopped: (s) => stops.push(s), log() {} });
  const named = new Map<number, LinkObject>();
  mount_stop({
    mount_named: (h: number, o: LinkObject) => named.set(h, o),
  } as unknown as Side);
  // The stop's args, as the guest's binding lays them (wit/stop).
  const args = pkt_of([
    48,
    (w) => {
      w.i32(0, ERRNO_EIO);
      w.str(4, long);
      w.u32(12, 7);
      w.u64(16, 1n);
      w.u64(24, 2n);
      w.u8(32, 1);
      w.str(36, long);
    },
  ])!;
  const ret = new Lower(new Uint8Array(0), 0);
  assert.equal(named.get(HANDLE)!.call(STOPPED, 0, new Lift(args), ret), 0);
  assert.deepEqual(stops, [
    { errnum: ERRNO_EIO, file: cut, line: 7, n1: 1n, n2: 2n, msg: cut },
  ]);
});

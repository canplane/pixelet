// @xpute/runtime-web/worker.test.ts

/** The runtime enters the guest only once every interface its world imports
 * is mounted (worker.ts `enter`), as xpute-runtime-native link.test.rs holds
 * a device's. */

import { test, vi } from "vitest";
import assert from "node:assert/strict";

import { Side, uri } from "@xpute/link";
import { NAMED } from "@xpute/link/link.ts";
import type { Trap } from "./message.ts";
import { run } from "./worker.ts";

test("a world importing what no server mounts refuses to start, naming each, and the guest is never entered", () => {
  const told: unknown[] = [];
  vi.stubGlobal("postMessage", (m: unknown) => told.push(m));
  let entered = false;
  const side = new Side({ mem: new ArrayBuffer(8), in: () => 0 });
  const g = {
    side,
    init: () => {
      entered = true;
      return 0n;
    },
    boot: () => {},
  };
  const [named, opened, missing, missing_too] = [0x30, 0x31, 0x32, 0x33].map(
    (k) => (NAMED | k) >>> 0,
  ) as [number, number, number, number];
  const { port1, port2 } = new MessageChannel();
  run(
    g,
    {
      servers: [
        (s) => s.mount_named(named, { call: () => 0, drop() {} }),
        (s) =>
          s.mount(opened, uri("opened"), () => ({ call: () => 0, drop() {} })),
      ],
      imports: [
        ["named", named],
        ["opened", opened],
        ["missing", missing],
        ["missing-too", missing_too],
      ],
      calls: [],
      taker: { give() {}, take() {} },
    },
    { port: port1, named: [], prefixes: [] },
  );
  port1.close();
  port2.close();
  const trap: Trap = {
    kind: "trap",
    err: "no server mounts missing, missing-too",
  };
  assert.deepEqual(told, [trap]);
  assert.equal(entered, false);
});

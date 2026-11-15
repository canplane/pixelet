// @xpute/runtime-web/worker.calls.test.ts

/** A function without `async` the UI serves across its port refuses to
 * start, naming its interface, and the guest is never entered (worker.ts
 * `enter`), as xpute-runtime-native link.calls.test.rs holds a device's. */

import { test, vi } from "vitest";
import assert from "node:assert/strict";

import { Side } from "@xpute/link";
import { NAMED } from "@xpute/link/link.ts";
import type { Trap } from "./message.ts";
import { run } from "./worker.ts";

test("a function without async served by the UI refuses to start, naming its interface, and the guest is never entered", () => {
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
  // An interface the UI serves across the port, one of whose methods
  // answers at once.
  const called = (NAMED | 0x30) >>> 0;
  const { port1, port2 } = new MessageChannel();
  run(
    g,
    {
      servers: [],
      imports: [["called", called]],
      calls: [[called, 0]],
      taker: { give() {}, take() {} },
    },
    { port: port1, named: [called], prefixes: [] },
  );
  port1.close();
  port2.close();
  const trap: Trap = {
    kind: "trap",
    err: "a function without async is served by the UI: called",
  };
  assert.deepEqual(told, [trap]);
  assert.equal(entered, false);
});

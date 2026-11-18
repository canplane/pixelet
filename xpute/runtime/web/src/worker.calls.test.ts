// @xpute/runtime-web/worker.calls.test.ts

/** A function without `async` the UI implements across its port refuses to
 * start, naming its interface, and the guest is never entered (worker.ts
 * `enter`), as xpute-runtime-native link.calls.test.rs holds the native
 * runtime's. */

import { test, vi } from "vitest";
import assert from "node:assert/strict";

import { NAMED, Side } from "@xpute/link/link.ts";
import { LEVEL_ERROR } from "@xpute/host/abi/log/log.enum.ts";
import type { Log, Trap } from "./message.ts";
import { run } from "./worker.ts";

test("a function without async implemented by the UI refuses to start, naming its interface, and the guest is never entered", () => {
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
  // An interface the UI implements across the port, one of whose methods
  // answers at once.
  const called = (NAMED | 0x30) >>> 0;
  const { port1, port2 } = new MessageChannel();
  run(
    g,
    {
      mounts: [],
      imports: [["called", called]],
      calls: [[called, 0]],
      taker: { give() {}, take() {} },
    },
    { port: port1, named: [called], prefixes: [] },
  );
  port1.close();
  port2.close();
  const err = "a function without async is implemented by the UI: called";
  const written: Log = {
    kind: "log",
    level: LEVEL_ERROR,
    context: "link",
    msg: `trapped: ${err}`,
  };
  const trap: Trap = { kind: "trap", err };
  assert.deepEqual(told, [written, trap]);
  assert.equal(entered, false);
});

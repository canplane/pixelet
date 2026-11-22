// @xpute/runtime-web/worker.log.test.ts

/** The runtime's writer cuts each record it writes, the host's and its own
 * (worker.ts `log`), as the record of the cut holds it
 * (kit/spec/golden/status/msg.tsv). */

import { test, vi } from "vitest";
import assert from "node:assert/strict";

import { Golden } from "@xpute/golden";
import { td, te } from "@xpute/kit/codec/encoding.ts";
import { ERRNO_EIO } from "@xpute/kit/status/errno.spec.ts";
import { MSG_CAP } from "@xpute/kit/status/msg.ts";
import { NAMED, Side } from "@xpute/link/link.ts";
import { LEVEL_WARN } from "@xpute/host/abi/log/log.enum.ts";
import { log } from "@xpute/host/log.ts";
import { make_stop, runtime } from "@xpute/host/runtime.ts";
import type { Log, Stopped } from "./message.ts";
import { run } from "./worker.ts";

test("the writer cuts a record's context and message, and a stop's line, past the cap", async () => {
  const g = await Golden.load("kit/spec/golden/status/msg.tsv");
  const [long, cut] = ["straddle.in", "straddle.out"].map((k) =>
    td.decode(g.bytes(k)),
  ) as [string, string];
  const told: unknown[] = [];
  vi.stubGlobal("postMessage", (m: unknown) => told.push(m));
  // A world that imports what nothing implements is refused, once the host
  // has its runtime, and its writer with it.
  const side = new Side({ mem: new ArrayBuffer(8), in: () => 0 });
  const { port1, port2 } = new MessageChannel();
  run(
    { side, init: () => 0n, boot: () => {} },
    {
      mounts: [],
      imports: [["missing", (NAMED | 0x30) >>> 0]],
      calls: [],
      taker: { give() {}, take() {} },
    },
    { port: port1, named: [], prefixes: [] },
  );
  port1.close();
  port2.close();
  vi.stubGlobal("console", { ...console, warn() {}, error() {} });
  told.length = 0;

  log(LEVEL_WARN, long, long);
  const written: Log = {
    kind: "log",
    level: LEVEL_WARN,
    context: cut,
    msg: cut,
  };
  assert.deepEqual(told, [written]);

  told.length = 0;
  runtime().stopped(make_stop(ERRNO_EIO, long, 7, 1n, 2n, long));
  const [line, stopped] = told as [Log, Stopped];
  assert.equal(line.context, "guest");
  assert.ok(line.msg.startsWith("stopped: "), line.msg);
  assert.ok(te.encode(line.msg).length <= MSG_CAP);
  assert.equal(stopped.stop.file, cut);
});

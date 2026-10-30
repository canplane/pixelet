// @xpute/example/sum.test.ts

/**
 * The smallest host, beside the smallest guest (rust/example): the guest's
 * module opened over a memory of its own, the host mounted on it with a
 * policy for a host with no display and an answer to the one hostcall the
 * guest makes, then one command sent and its reply read. Build the guest
 * first (`make build`).
 */

import { test } from "vitest";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { mount } from "@xpute/host/host.ts";
import { op } from "@xpute/host/ipc/sys.spec.ts";
import { open_module } from "@xpute/host/module.ts";
import { FixedQuota } from "@xpute/host/sched/fixed_quota.ts";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
/** A relative CARGO_TARGET_DIR is cargo's, which the root's tasks run from rust/. */
const CARGO_TARGET = resolve(
  join(ROOT, "rust"),
  process.env.CARGO_TARGET_DIR ?? "target",
);
const GUEST = join(
  CARGO_TARGET,
  "wasm32-unknown-unknown/debug/xpute_example.wasm",
);

// The guest's numbers (rust/example/src/lib.rs).
const SUM = 0x1001;
const ASK = (op.PROGRAM | 1) >>> 0;

/** What the host holds and the guest asks for. */
const HELD = 100;

/** The port the program sends as: any but the host's, 0. */
const SENDER = 1;

test("example - a command answered with what the guest asked its host", () => {
  // Two MiB, never grown: the guest's stack, its data, its range and its heap.
  const memory = new WebAssembly.Memory({ initial: 32, maximum: 32 });
  const guest = open_module(
    new WebAssembly.Module(readFileSync(GUEST)),
    memory,
  );
  const boot = guest.instance.exports.boot as () => number;
  const host = mount(guest, {
    directory_at: boot() >>> 0,
    sys: { program: (code) => (code === ASK ? HELD : Errno.ENOSYS) },
    policy: new FixedQuota(4),
    submit: () => {},
  });

  const pkt = host.doorbell.writer(2);
  pkt.u32(2);
  pkt.u32(3);
  assert.deepStrictEqual(
    host.doorbell.call(SENDER, SUM, pkt, (result) => result),
    2 + 3 + HELD,
  );
});

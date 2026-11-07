// @xpute/runtime/start.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import type { Hello } from "./remote.ts";
import type { Manifest, Ui } from "./runtime.ts";
import { start } from "./start.ts";

const MANIFEST = { machine: { slot_bytes: 64 } } as unknown as Manifest;

/** A worker that keeps the port it is sent. */
function worker(): Worker & { port: Promise<MessagePort> } {
  const port = Promise.withResolvers<MessagePort>();
  return {
    port: port.promise,
    postMessage: (m: Hello) => port.resolve(m.xpute),
  } as unknown as Worker & { port: Promise<MessagePort> };
}

/** A UI that counts its stops. */
function ui(): Ui & { stops: number } {
  return {
    cmds: [],
    stops: 0,
    on() {},
    gpu() {},
    stop() {
      this.stops++;
    },
  };
}

const error = new Event("error");

test("start - a worker that fails before the core rose refuses the link, and stops nothing", async () => {
  const w = worker();
  const u = ui();
  const link = start(MANIFEST, u, () => w);
  w.onerror!(error as ErrorEvent);
  await assert.rejects(link, /the core's worker: error/);
  assert.deepStrictEqual(u.stops, 0);
  (await w.port).close();
});

test("start - a worker that fails after the core rose is the core's stop, told once", async () => {
  const w = worker();
  const u = ui();
  const up = start(MANIFEST, u, () => w);
  const port = await w.port;
  port.postMessage({ kind: "rose", why: null });
  const link = await up;
  const waiting = link.request(0x1001);
  w.onerror!(error as ErrorEvent);
  w.onmessageerror!(new MessageEvent("messageerror"));
  assert.deepStrictEqual(await waiting, Errno.ENOTRECOVERABLE);
  assert.deepStrictEqual(u.stops, 1);
  port.close();
});

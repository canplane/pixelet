// @xpute/runtime/remote.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import type { XputeError } from "@xpute/kit/status/error.ts";
import { PacketWriter, TreeReader } from "@xpute/kit/wire/xtp/mod.ts";
import type { Encode } from "./io/connection.ts";
import { attach, connect } from "./remote.ts";
import type { Runtime, Ui } from "./runtime.ts";

/** This runtime keeps what it is sent, each packet read back as its words,
 * and answers a request with its command plus one. */
function runtime(): {
  rt: Runtime;
  sent: [number, number[]][];
} {
  const sent: [number, number[]][] = [];
  const bytes = new Uint8Array(256);
  const view = new DataView(bytes.buffer);
  const writer = new PacketWriter();
  const read = (encode?: Encode | null): number[] => {
    const w = encode?.({
      writer: (cnt) => writer.begin(bytes, view, 0, bytes.length, cnt),
    });
    if (!w) return [];
    const end = w.finish();
    const f = new TreeReader(bytes.subarray(0, end)).read_branch();
    return [f.at(0).get<number>(), f.at(1).get<number>()];
  };
  const rt = {
    connection: {
      send: (cmd: number, encode: Encode | null) =>
        void sent.push([cmd, read(encode)]),
      request: (cmd: number, encode: Encode | null) => {
        sent.push([cmd, read(encode)]);
        return Promise.resolve(cmd + 1);
      },
    },
    kick: () => {},
  } as unknown as Runtime;
  return { rt, sent };
}

/** This UI keeps what comes down on `majors`. */
function ui(majors: number[] = []): Ui & { heard: unknown[] } {
  const heard: unknown[] = [];
  return {
    protocols: [
      {
        majors,
        on: (_tag, cmd, f, _pkt, named) =>
          void heard.push([cmd, f?.at(0).get<number>(), [...(named ?? [])]]),
      },
    ],
    heard,
    stop: () => heard.push("stop"),
  };
}

const tick = () => new Promise((done) => setTimeout(done, 0));

test("remote - a UI's send and request reach the runtime as they were encoded", async () => {
  const { port1, port2 } = new MessageChannel();
  const { rt, sent } = runtime();
  attach(port2, []).serve(rt);
  const { link } = connect(port1, 256, ui());
  link.send(0x1001, 7, 9);
  assert.deepStrictEqual(await link.request(0x1002, 1, 2), 0x1003);
  assert.deepStrictEqual(sent, [
    [0x1001, [7, 9]],
    [0x1002, [1, 2]],
  ]);
  port1.close();
});

test("remote - an import the UI takes comes down with its packet and the memory it names", async () => {
  const { port1, port2 } = new MessageChannel();
  const end = attach(port2, [0x11]);
  const u = ui([0x11]);
  connect(port1, 256, u);
  const bytes = new Uint8Array(64);
  const w = new PacketWriter().begin(
    bytes,
    new DataView(bytes.buffer),
    0,
    64,
    1,
  );
  const pkt = bytes.subarray(0, w.u32(42).finish());
  end.ui.protocols[0].on(0, 0x1101, null, pkt, new Uint8Array([4, 5, 6]));
  await tick();
  assert.deepStrictEqual(u.heard, [[0x1101, 42, [4, 5, 6]]]);
  port1.close();
});

test("remote - once the core stops, a request waiting and every one after reject with ENOTRECOVERABLE", async () => {
  const { port1, port2 } = new MessageChannel();
  const rt = {
    connection: {
      send: () => {},
      request: () => new Promise<number>(() => {}),
    },
    kick: () => {},
  } as unknown as Runtime;
  const end = attach(port2, []);
  end.serve(rt);
  const u = ui();
  const { link } = connect(port1, 256, u);
  const waiting = link.request(0x1001);
  end.ui.stop();
  await assert.rejects(
    waiting,
    (e: XputeError) => e.errnum === Errno.ENOTRECOVERABLE,
  );
  await assert.rejects(
    link.request(0x1001),
    (e: XputeError) => e.errnum === Errno.ENOTRECOVERABLE,
  );
  assert.deepStrictEqual(u.heard, ["stop"]);
  port1.close();
});

test("remote - the UI's calls reach the runtime by name", async () => {
  const { port1, port2 } = new MessageChannel();
  const asked: string[] = [];
  const paused = Promise.withResolvers<void>();
  const rt = {
    wake: () => asked.push("wake"),
    resume: () => asked.push("resume"),
    pause: () => (asked.push("pause"), paused.resolve()),
    visible: (v: boolean) => asked.push(`visible ${v}`),
  } as unknown as Runtime;
  const end = attach(port2, []);
  end.serve(rt);
  const u = ui();
  const { link } = connect(port1, 256, u);
  link.resume();
  assert.ok(link.shown(), "shown until told otherwise");
  link.visible(false);
  assert.ok(!link.shown(), "hidden as last told");
  link.wake();
  link.pause();
  await paused.promise;
  await tick();
  assert.deepStrictEqual(asked, ["resume", "visible false", "wake", "pause"]);
  port1.close();
});

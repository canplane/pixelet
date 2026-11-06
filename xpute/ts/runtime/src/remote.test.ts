// @xpute/runtime/remote.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import { PacketWriter, TreeReader } from "@xpute/kit/wire/xtp/mod.ts";
import type { Encode } from "./ipc/doorbell.ts";
import { connect, serve, close } from "./remote.ts";
import type { Runtime } from "./runtime.ts";

/** A runtime that keeps what it is sent, each packet read back as its words,
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
    send: (cmd: number, encode?: Encode) => void sent.push([cmd, read(encode)]),
    request: (cmd: number, encode?: Encode | null) => {
      sent.push([cmd, read(encode)]);
      return Promise.resolve(cmd + 1);
    },
  } as unknown as Runtime;
  return { rt, sent };
}

const two =
  (a: number, b: number): Encode =>
  (room) =>
    room.writer(2).u32(a).u32(b);

test("remote - a UI's send and request reach the runtime as they were encoded", async () => {
  const { port1, port2 } = new MessageChannel();
  const { rt, sent } = runtime();
  serve(rt, port2);
  const ui = connect(port1, 256);
  ui.send(0x1001, two(7, 9));
  assert.deepStrictEqual(await ui.request(0x1002, two(1, 2)), 0x1003);
  assert.deepStrictEqual(sent, [
    [0x1001, [7, 9]],
    [0x1002, [1, 2]],
  ]);
  assert.throws(() => ui.call(0x1003), InvariantError);
  port1.close();
});

test("remote - an import placed with the UI comes with its packet and the memory it names", async () => {
  const { port1, port2 } = new MessageChannel();
  const { rt } = runtime();
  const raise = serve(rt, port2, (cmd) =>
    cmd === 0x1101 ? new Uint8Array([4, 5, 6]) : null,
  );
  const ui = connect(port1, 256);
  const heard = new Promise<[number, number, number[]]>((resolve) =>
    ui.listen((_tag, cmd, f, named) =>
      resolve([cmd, f?.at(0).get<number>() ?? -1, [...(named ?? [])]]),
    ),
  );
  const bytes = new Uint8Array(64);
  const w = new PacketWriter().begin(
    bytes,
    new DataView(bytes.buffer),
    0,
    64,
    1,
  );
  const pkt = bytes.subarray(0, w.u32(42).finish());
  raise(0, 0x1101, new TreeReader(pkt).read_branch(), pkt);
  assert.deepStrictEqual(await heard, [0x1101, 42, [4, 5, 6]]);
  port1.close();
});

test("remote - once the core stops, a request waiting and every one after answer ENOTRECOVERABLE", async () => {
  const { port1, port2 } = new MessageChannel();
  const rt = {
    send: () => {},
    request: () => new Promise<number>(() => {}),
  } as unknown as Runtime;
  serve(rt, port2);
  const ui = connect(port1, 256);
  const waiting = ui.request(0x1001);
  const told = new Promise<void>((resolve) => (ui.onterminated = resolve));
  close(port2);
  await told;
  assert.deepStrictEqual(await waiting, Errno.ENOTRECOVERABLE);
  assert.deepStrictEqual(await ui.request(0x1001), Errno.ENOTRECOVERABLE);
  port1.close();
});

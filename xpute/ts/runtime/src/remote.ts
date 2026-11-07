// @xpute/runtime/remote.ts

/**
 * A UI in another thread than its core and runtime, as a page's main thread is
 * beside a worker that holds them: the UI's frames cross a MessagePort and
 * nothing else does, since the two share no memory. The UI's end (`connect`)
 * is a `Sender`, so a protocol's generated stubs bind to it as they bind to a
 * runtime beside them; it posts each command's packet as bytes, encoded in
 * its own memory. The runtime's end (`serve`) submits each on the SQ as it
 * came, and posts back a request's reply and every import's frame the program places
 * with the UI, its packet copied before the runtime's head passes it, with
 * what else of the core's memory the program says the frame names.
 *
 * A call waits on a turn, so a UI across a port makes none: `call` is
 * refused with ENOTSUP. Once the core stops, every request waiting and
 * every one after is answered ENOTRECOVERABLE.
 */

import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import type { U8Array } from "@xpute/kit/abi/array.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import {
  type BranchCursor,
  type PacketRoom,
  PacketWriter,
  TreeReader,
} from "@xpute/kit/wire/xtp/mod.ts";
import type { Encode, OnImport } from "./ipc/doorbell.ts";
import type { Runtime, Sender } from "./runtime.ts";

/** A UI's frame: a command, `id` a request's (0 for a send), and its packet,
 * `cnt` children laid as `pkt`. */
interface FrameMessage {
  kind: "frame";
  cmd: u32;
  id: u32;
  cnt: u32;
  pkt: U8Array | null;
}

/** A request's answer, by its `id`. */
interface ReplyMessage {
  kind: "reply";
  id: u32;
  result: i32;
}

/** An import the program placed with the UI: its packet, and the bytes of
 * the core's memory it names, as the program copied them. */
interface ImportMessage {
  kind: "import";
  tag: u32;
  cmd: u32;
  pkt: U8Array | null;
  named: U8Array | null;
}

/** The core stopped. */
interface StopMessage {
  kind: "stop";
}

/** What the UI hears of an import's frame placed with it: as `OnImport`, with the
 * bytes of the core's memory the frame names, the UI's to keep. */
export type OnUiImport = (
  tag: u32,
  cmd: u32,
  f: BranchCursor | null,
  named: U8Array | null,
) => void;

/** The UI's end of a port to its runtime. */
export interface Remote extends Sender {
  /** What the core raises for the UI is handed to `listener`. */
  listen(listener: OnUiImport): void;
  /** Told once, as the core stops. */
  onterminated: (() => void) | null;
}

/** The UI's end of `port`; `slot_bytes` is the machine's, the most a packet
 * may take in the SQ. */
export function connect(port: MessagePort, slot_bytes: u32): Remote {
  const scratch = new Uint8Array(slot_bytes);
  const view = new DataView(scratch.buffer);
  const writer = new PacketWriter();
  let cnt = 0;
  const room: PacketRoom = {
    writer(n: u32): PacketWriter {
      cnt = n;
      return writer.begin(scratch, view, 0, scratch.length, n);
    },
  };
  const waiting = new Map<u32, (result: i32) => void>();
  let next_id: u32 = 0;
  let stopped = false;
  let listener: OnUiImport = () => {};

  function post(cmd: u32, id: u32, encode: Encode | null | undefined): void {
    cnt = 0;
    const w = encode?.(room) ?? null;
    let pkt: U8Array | null = null;
    if (w !== null) {
      const end = w.finish();
      if (end < 0) throw new InvariantError(Errno.EMSGSIZE);
      pkt = scratch.slice(0, end);
    }
    const frame: FrameMessage = { kind: "frame", cmd, id, cnt, pkt };
    port.postMessage(frame, pkt ? [pkt.buffer] : []);
  }

  const remote: Remote = {
    send(cmd, encode) {
      if (!stopped) post(cmd, 0, encode);
    },
    request(cmd, encode) {
      if (stopped) return Promise.resolve(Errno.ENOTRECOVERABLE);
      next_id = (next_id + 1) >>> 0 || 1;
      const id = next_id;
      return new Promise((resolve) => {
        waiting.set(id, resolve);
        post(cmd, id, encode);
      });
    },
    call(): never {
      throw new InvariantError(Errno.ENOTSUP);
    },
    listen(l) {
      listener = l;
    },
    onterminated: null,
  };

  port.onmessage = ({
    data,
  }: MessageEvent<ReplyMessage | ImportMessage | StopMessage>) => {
    if (data.kind === "reply") {
      const answer = waiting.get(data.id);
      waiting.delete(data.id);
      answer?.(data.result);
    } else if (data.kind === "import") {
      const f = data.pkt ? new TreeReader(data.pkt).read_branch() : null;
      listener(data.tag, data.cmd, f, data.named);
    } else {
      stopped = true;
      for (const answer of waiting.values()) answer(Errno.ENOTRECOVERABLE);
      waiting.clear();
      remote.onterminated?.();
    }
  };
  return remote;
}

/** What else of the core's memory an import's frame names, copied while it
 * is read; null for none. */
export type MemoryOf = (cmd: u32, f: BranchCursor | null) => U8Array | null;

/** The runtime's end of `port`: the UI's frames submitted as they came. The
 * returned listener is what the program hands the imports it places with the
 * UI: each is posted, its packet and what `named` copies of it. */
export function serve(
  runtime: Runtime,
  port: MessagePort,
  memory_of: MemoryOf = () => null,
): OnImport {
  port.onmessage = ({ data }: MessageEvent<FrameMessage>) => {
    const { cmd, id, cnt, pkt } = data;
    const encode: Encode | null = pkt
      ? (room) => room.writer(cnt).copy(pkt)
      : null;
    if (id === 0) return runtime.send(cmd, encode ?? undefined);
    void runtime.request(cmd, encode).then((result) => {
      const reply: ReplyMessage = { kind: "reply", id, result };
      port.postMessage(reply);
    });
  };
  return (tag, cmd, f, pkt) => {
    const copy = pkt ? pkt.slice() : null;
    const more = memory_of(cmd, f);
    const raised: ImportMessage = {
      kind: "import",
      tag,
      cmd,
      pkt: copy,
      named: more,
    };
    const moved: ArrayBuffer[] = [];
    if (copy) moved.push(copy.buffer);
    if (more) moved.push(more.buffer as ArrayBuffer);
    port.postMessage(raised, moved);
  };
}

/** Tells the UI's end the core stopped. */
export function close(port: MessagePort): void {
  const stop: StopMessage = { kind: "stop" };
  port.postMessage(stop);
}

// @xpute/runtime/remote.ts

/**
 * A UI apart from its core, as a page's main thread is from the worker that
 * runs it: a MessagePort between them, which carries frames.
 *
 * - Up: the UI's frames, a lifecycle call by name, a canvas transferred once.
 * - Down: frames, a reply to a request or an import the UI takes, with the
 *   memory it names copied; the GPU's state; the stop; whether the core rose.
 * - A call waits on a turn, so a UI across a port makes none (ENOTSUP). Once
 *   the core stops, every request is answered ENOTRECOVERABLE.
 */

import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import type { U8Array } from "@xpute/kit/abi/array.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import {
  type PacketRoom,
  PacketWriter,
  TreeReader,
} from "@xpute/kit/wire/xtp/mod.ts";
import type { Encode } from "./ipc/doorbell.ts";
import {
  type Canvas,
  type GpuState,
  type Link,
  type Manifest,
  boot,
  type Runtime,
  type Ui,
} from "./runtime.ts";
import type { Grant } from "./sched/policy.ts";

/** The calls a link carries by name. */
const CALLS = [
  "wake",
  "input",
  "resume",
  "pause",
  "visible",
  "unbind",
] as const;

type Call = (typeof CALLS)[number];

type Up =
  | { kind: "frame"; cmd: u32; id: u32; cnt: u32; pkt: U8Array | null }
  | { kind: "call"; name: Call; arg: unknown }
  | { kind: "bind"; handle: u32; canvas: OffscreenCanvas };

/** `res` a reply's result, null for an import. */
type Down =
  | {
      kind: "frame";
      tag: u32;
      cmd: u32;
      res: i32 | null;
      pkt: U8Array | null;
      named: U8Array | null;
    }
  | { kind: "gpu"; state: GpuState }
  | { kind: "stop" }
  | { kind: "rose"; why: string | null };

/** What `start` sends the worker first. */
export interface Hello {
  xpute: MessagePort;
  cmds: readonly u32[];
}

/** The UI's end of `port`, what comes down handed to `ui`; `rose` settles
 * once the core is up, and `stop` is the core's stop where the worker that
 * ran it died with no word of its own. */
export function connect(
  port: MessagePort,
  slot_bytes: u32,
  ui: Ui,
): { link: Link; rose: Promise<void>; stop(): void } {
  const buf = new Uint8Array(slot_bytes);
  const view = new DataView(buf.buffer);
  const w = new PacketWriter();
  const room: PacketRoom = {
    writer: (n) => ((cnt = n), w.begin(buf, view, 0, buf.length, n)),
  };
  const waiting = new Map<u32, (res: i32) => void>();
  const rose = Promise.withResolvers<void>();
  const up = (m: Up, moved: Transferable[] = []) => port.postMessage(m, moved);
  let cnt = 0;
  let next = 0;
  let stopped = false;

  function frame(cmd: u32, id: u32, encode?: Encode | null): void {
    cnt = 0;
    const end = encode?.(room)?.finish() ?? 0;
    if (end < 0) throw new InvariantError(Errno.EMSGSIZE);
    const pkt = end > 0 ? buf.slice(0, end) : null;
    up({ kind: "frame", cmd, id, cnt, pkt }, pkt ? [pkt.buffer] : []);
  }

  function bind(handle: u32, canvas: Canvas): void {
    const off =
      canvas instanceof OffscreenCanvas
        ? canvas
        : canvas.transferControlToOffscreen();
    up({ kind: "bind", handle, canvas: off }, [off]);
  }

  function stop(): void {
    if (stopped) return;
    stopped = true;
    waiting.forEach((done) => done(Errno.ENOTRECOVERABLE));
    waiting.clear();
    ui.stop();
  }

  const call = (name: Call, arg?: unknown) => up({ kind: "call", name, arg });

  port.onmessage = ({ data: m }: MessageEvent<Down>) => {
    switch (m.kind) {
      case "frame":
        if (m.res !== null) {
          waiting.get(m.tag)?.(m.res);
          return void waiting.delete(m.tag);
        }
        return ui.on(
          m.tag,
          m.cmd,
          m.pkt && new TreeReader(m.pkt).read_branch(),
          m.named,
          m.pkt,
        );
      case "gpu":
        return ui.gpu(m.state);
      case "stop":
        return stop();
      case "rose":
        return m.why === null ? rose.resolve() : rose.reject(new Error(m.why));
    }
  };

  const link: Link = {
    send: (cmd, encode) => void (stopped || frame(cmd, 0, encode)),
    request(cmd, encode) {
      if (stopped) return Promise.resolve(Errno.ENOTRECOVERABLE);
      const id = (next = (next + 1) >>> 0 || 1);
      return new Promise((done) => {
        waiting.set(id, done);
        frame(cmd, id, encode);
      });
    },
    call(): never {
      throw new InvariantError(Errno.ENOTSUP);
    },
    fits: (encode) => (encode(room)?.finish() ?? 0) >= 0,
    wake: () => call("wake"),
    input: () => call("input"),
    resume: () => call("resume"),
    pause: () => call("pause"),
    visible: (v) => call("visible", v),
    bind,
    unbind: (handle) => call("unbind", handle),
  };
  return { link, rose: rose.promise, stop };
}

/** The worker's end of `port`: its runtime `r`, what the UI takes posted down
 * through the `Ui` it returns. */
export function attach(
  port: MessagePort,
  cmds: readonly u32[],
): {
  ui: Ui;
  serve(r: Runtime): void;
} {
  const down = (m: Down, moved: Transferable[] = []) =>
    port.postMessage(m, moved);
  const ui: Ui = {
    cmds,
    on(tag, cmd, _f, named, pkt) {
      const p = pkt?.slice() ?? null;
      const n = named?.slice() ?? null;
      const moved = [p, n].flatMap((b) => (b ? [b.buffer] : []));
      down({ kind: "frame", tag, cmd, res: null, pkt: p, named: n }, moved);
    },
    gpu: (state) => down({ kind: "gpu", state }),
    stop: () => down({ kind: "stop" }),
  };
  function serve(r: Runtime): void {
    port.onmessage = ({ data: m }: MessageEvent<Up>) => {
      switch (m.kind) {
        case "frame": {
          const { cmd, id, cnt, pkt } = m;
          const encode =
            pkt && ((room: PacketRoom) => room.writer(cnt).copy(pkt));
          if (id === 0) return r.send(cmd, encode ?? undefined);
          return void r.request(cmd, encode).then((res) =>
            down({
              kind: "frame",
              tag: id,
              cmd,
              res,
              pkt: null,
              named: null,
            }),
          );
        }
        case "call":
          return (r[m.name] as (arg: unknown) => void)(m.arg);
        case "bind":
          return r.bind(m.handle, m.canvas);
      }
    };
  }
  return { ui, serve };
}

/** A page's worker serving the core of `m`: it takes the port `start` sends,
 * raises the core, and serves the UI's frames. A message of the worker's own
 * passes by. */
export function serve<G extends Grant>(m: Manifest<G>): Promise<Runtime> {
  return new Promise((done, fail) => {
    const hello = async ({ data }: MessageEvent) => {
      const h = data as Partial<Hello> | null;
      if (!(h?.xpute instanceof MessagePort)) return;
      removeEventListener("message", hello);
      const port = h.xpute;
      const end = attach(port, h.cmds ?? []);
      try {
        const r = await boot(m, end.ui);
        end.serve(r);
        port.postMessage({ kind: "rose", why: null } satisfies Down);
        done(r);
      } catch (err) {
        const why = String((err as Error)?.message ?? err);
        port.postMessage({ kind: "rose", why } satisfies Down);
        fail(err);
      }
    };
    addEventListener("message", hello);
  });
}

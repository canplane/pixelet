// @xpute/runtime/remote.ts

/**
 * This is a UI apart from its core, as a page's main thread is apart from the
 * worker that runs the core, with a MessagePort between them that carries
 * frames.
 *
 * - Up the port go the UI's frames, and a lifecycle call by name.
 * - Down the port come frames (a reply to a request, or an import the UI
 *   takes, with the memory it names copied), the lines the worker says
 *   (say.ts), the stop, and whether the core came up.
 * - Once the core stops, every request is answered ENOTRECOVERABLE.
 */

import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import type { U8Array } from "@xpute/kit/abi/array.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { InvariantError, XputeError } from "@xpute/kit/status/error.ts";
import {
  type Arg,
  type PacketRoom,
  PacketWriter,
  TreeReader,
} from "@xpute/kit/wire/xtp/mod.ts";
import { answer_of, encode_args } from "./io/connection.ts";
import { major_of as frame_major } from "./io/frame.ts";
import { say, say_through } from "./say.ts";
import {
  boot,
  type Config,
  type Link,
  type Manifest,
  routes_of,
  type Runtime,
  type Ui,
} from "./runtime.ts";
import type { Grant } from "./sched/policy.ts";

/** These are the calls a link carries by name. */
const CALLS = ["wake", "input", "resume", "pause", "visible"] as const;

type Call = (typeof CALLS)[number];

type Up =
  | { kind: "frame"; cmd: u32; id: u32; cnt: u32; pkt: U8Array | null }
  | { kind: "call"; name: Call; arg: unknown };

/** A frame's `res` is a reply's result, or null for an import. */
type Down =
  | {
      kind: "frame";
      tag: u32;
      cmd: u32;
      res: i32 | null;
      pkt: U8Array | null;
      named: U8Array | null;
    }
  | { kind: "say"; line: string }
  | { kind: "stop" }
  | { kind: "rose"; why: string | null };

/** This is what `start` sends the worker first. `origin` is the UI's
 * `performance.timeOrigin`, so that the worker can count the core's clock as
 * the UI's `performance.now()` counts it. */
export interface Hello {
  xpute: MessagePort;
  /** The majors the UI takes (`Ui.protocols`), whose frames go down. */
  majors: readonly u32[];
  origin: f64;
}

/** This is the UI's end of `port`, and what comes down is handed to `ui`.
 * `rose` settles once the core is up. `stop` ends the link as the core's stop,
 * for when the worker that ran the core died without a word of its own. */
export function connect(
  port: MessagePort,
  pkt_bytes: u32,
  ui: Ui,
): { link: Link; rose: Promise<void>; stop(): void } {
  const buf = new Uint8Array(pkt_bytes);
  const view = new DataView(buf.buffer);
  const w = new PacketWriter();
  const room: PacketRoom = {
    writer: (n) => ((cnt = n), w.begin(buf, view, 0, buf.length, n)),
  };
  const waiting = new Map<u32, (res: i32) => void>();
  const routes = routes_of(ui.protocols);
  const rose = Promise.withResolvers<void>();
  const up = (m: Up, moved: Transferable[] = []) => port.postMessage(m, moved);
  let cnt = 0;
  let next = 0;
  let stopped = false;

  function frame(cmd: u32, id: u32, args: Arg[]): void {
    cnt = 0;
    const end = encode_args(args)(room)?.finish() ?? 0;
    if (end < 0) throw new InvariantError(Errno.EMSGSIZE);
    const pkt = end > 0 ? buf.slice(0, end) : null;
    up({ kind: "frame", cmd, id, cnt, pkt }, pkt ? [pkt.buffer] : []);
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
        return void routes.get(frame_major(m.cmd))?.(
          m.tag,
          m.cmd,
          m.pkt && new TreeReader(m.pkt).read_branch(),
          m.pkt,
          m.named,
        );
      case "say":
        return say(m.line);
      case "stop":
        return stop();
      case "rose":
        return m.why === null ? rose.resolve() : rose.reject(new Error(m.why));
    }
  };

  // The runtime across the port is told each change, and this end keeps the
  // last one for `shown`.
  let shown = true;
  const link: Link = {
    send: (cmd, ...args) => void (stopped || frame(cmd, 0, args)),
    request(cmd, ...args) {
      if (stopped) return Promise.reject(new XputeError(Errno.ENOTRECOVERABLE));
      const id = (next = (next + 1) >>> 0 || 1);
      return new Promise<i32>((done) => {
        waiting.set(id, done);
        frame(cmd, id, args);
      }).then(answer_of);
    },
    fits: (_cmd, ...args) => (encode_args(args)(room)?.finish() ?? 0) >= 0,
    wake: () => call("wake"),
    input: () => call("input"),
    resume: () => call("resume"),
    pause: () => call("pause"),
    visible: (v) => ((shown = v), call("visible", v)),
    shown: () => shown,
  };
  return { link, rose: rose.promise, stop };
}

/** This is the worker's end of `port`. The `Ui` it returns posts down what the
 * UI takes, the worker's lines go down from now on, and `serve` hands the UI's
 * frames and calls to the runtime `r`. */
export function attach(
  port: MessagePort,
  majors: readonly u32[],
): {
  ui: Ui;
  serve(r: Runtime): void;
} {
  const down = (m: Down, moved: Transferable[] = []) =>
    port.postMessage(m, moved);
  say_through((line) => down({ kind: "say", line }));
  const ui: Ui = {
    protocols: [
      {
        majors,
        on(tag, cmd, _f, pkt, named) {
          const p = pkt?.slice() ?? null;
          const n = named?.slice() ?? null;
          const moved = [p, n].flatMap((b) => (b ? [b.buffer] : []));
          down({ kind: "frame", tag, cmd, res: null, pkt: p, named: n }, moved);
        },
      },
    ],
    stop: () => down({ kind: "stop" }),
  };
  function serve(r: Runtime): void {
    port.onmessage = ({ data: m }: MessageEvent<Up>) => {
      switch (m.kind) {
        case "frame": {
          const { cmd, id, cnt, pkt } = m;
          const encode =
            pkt && ((room: PacketRoom) => room.writer(cnt).copy(pkt));
          if (id === 0) {
            r.connection.send(cmd, encode);
            return r.kick();
          }
          const answer = r.connection.request(cmd, encode);
          r.kick();
          return void answer.then((res) =>
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
      }
    };
  }
  return { ui, serve };
}

/** Serves the core `manifest` declares, with the config `core` makes, from a
 * page's worker. It takes the port `start` sends, brings the core up, and
 * serves the UI's frames. A message of the worker's own, which carries no port,
 * is left alone. */
export function serve<G extends Grant>(
  manifest: Manifest,
  core: () => Config<G> | Promise<Config<G>>,
): Promise<Runtime> {
  return new Promise((done, fail) => {
    const hello = async ({ data }: MessageEvent) => {
      const h = data as Partial<Hello> | null;
      if (!(h?.xpute instanceof MessagePort)) return;
      removeEventListener("message", hello);
      const port = h.xpute;
      const end = attach(port, h.majors ?? []);
      const shift =
        performance.timeOrigin - (h.origin ?? performance.timeOrigin);
      try {
        const r = await boot(
          manifest,
          core,
          end.ui,
          () => performance.now() + shift,
        );
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

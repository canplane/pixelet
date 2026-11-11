// @xpute/runtime/start.native.ts

/**
 * Attaches a device's UI to its runtime, which runs on a thread of its own in
 * Rust (xpute-runtime native/serve.rs), as a page's UI attaches to its worker
 * (remote.ts). What the UI sends crosses in one call each, and what it hears
 * comes in a batch when the runtime says there is one. What backs a file stays
 * here, with the program.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { InvariantError, XputeError } from "@xpute/kit/status/error.ts";
import {
  type Arg,
  type BranchCursor,
  type PacketRoom,
  PacketWriter,
  TreeReader,
} from "@xpute/kit/wire/xtp/mod.ts";
import { XPUTE } from "@xpute/native";
import { Command as FsCommand, read as fs_read } from "./abi/fs.spec.ts";
import { Command, sys_imports } from "./abi/sys.spec.ts";
import { encode_args, reply_of } from "./io/connection.ts";
import { commit, FileError } from "./io/fs.ts";
import * as batch from "./io/batch.spec.ts";
import { type Path, read_path } from "./io/path.ts";
import * as frame from "./io/frame.ts";
import { lay, laid } from "./io/head.ts";
import {
  abort_report,
  bind_mem,
  type Config,
  type Link,
  type Manifest,
  routes_of,
  type Ui,
} from "./runtime.ts";
import { FramePacing } from "./sched/frame_pacing.ts";
import type { Grant } from "./sched/policy.ts";

const NOTHING = new Uint8Array(0);

const padded = (n: u32) => Math.ceil(n / batch.ALIGN) * batch.ALIGN;

export async function start<G extends Grant>(
  manifest: Manifest,
  core: () => Config<G> | Promise<Config<G>>,
  ui: Ui,
): Promise<Link> {
  const cfg = await core();
  if (manifest.fs !== (cfg.fs !== undefined)) {
    throw new InvariantError(Errno.EINVAL);
  }
  // A device's runtime paces its turns to the display, by the program's
  // numbers, and runs no other policy.
  if (!(cfg.policy instanceof FramePacing)) {
    throw new InvariantError(Errno.ENOTSUP);
  }
  const pace = cfg.policy.cfg;
  const mem = XPUTE.mem();
  const at = manifest.head_at;
  if (!laid(mem, at)) lay(mem, at, manifest);
  bind_mem(mem);

  const buf = new Uint8Array(manifest.pkt_bytes);
  const view = new DataView(buf.buffer);
  const w = new PacketWriter();
  const room: PacketRoom = {
    writer: (n) => w.begin(buf, view, 0, buf.length, n),
  };
  const packet = (args: Arg[]): U8Array => {
    const end = encode_args(args)(room)?.finish() ?? 0;
    if (end < 0) throw new InvariantError(Errno.EMSGSIZE);
    return end > 0 ? buf.subarray(0, end) : NOTHING;
  };

  const waiting = new Map<u32, (res: i32, named: U8Array | null) => void>();
  let next = 0;
  let stopped = false;
  const post = (tag: u32, cmd: u32, flags: u32, res: i32, pkt: U8Array) =>
    XPUTE.post(tag, cmd, flags, res, pkt as Uint8Array);
  const send = (cmd: u32, ...args: Arg[]) => {
    if (!stopped) post(0, cmd, 0, 0, packet(args));
  };

  // The file device is served on the runtime's thread (native/serve.rs), and
  // what reaches the program of it is taken in `hear`.
  const routes = routes_of([
    sys_imports({
      [Command.SYS_ABORT]: (e) => console.error(abort_report(e)),
      // The runtime's thread takes the core's pace (native/serve.rs).
      [Command.SYS_PACE]: () => {},
    }),
    ...(cfg.program ?? []),
  ]);
  const ui_routes = routes_of(ui.protocols);

  /** Answers a frame as whatever takes its major does. */
  function answer(
    tag: u32,
    cmd: u32,
    f: BranchCursor | null,
    named: U8Array | null,
    pkt: U8Array | null,
  ): i32 | void {
    const major = frame.CMD_MAJOR(cmd);
    const on = routes.get(major);
    const take = ui_routes.get(major);
    take?.(tag, cmd, f, pkt, named);
    // What the UI alone takes is answered by nothing here.
    return on ? on(tag, cmd, f, pkt, named) : take ? undefined : Errno.ENOSYS;
  }

  /** Each open file's abort, which its close or a reload calls. */
  const files = new Map<u32, AbortController>();

  function open_file(fd: u32, path: Path): void {
    const file = new AbortController();
    files.set(fd, file);
    const failed = (err: unknown) => {
      if (files.get(fd) !== file) return;
      files.delete(fd);
      if (!(err instanceof FileError)) {
        console.warn(`[xpute/fs] open ${JSON.stringify(path)}: ${String(err)}`);
      }
      const [errnum, detail] =
        err instanceof FileError ? [err.errnum, err.detail] : [Errno.EIO, 0];
      XPUTE.fsCompleteOpen(fd, errnum, detail, NOTHING);
    };
    const got = (bytes: U8Array) => {
      if (files.get(fd) !== file) return;
      files.delete(fd);
      XPUTE.fsCompleteOpen(fd, bytes.byteLength, 0, bytes as Uint8Array);
    };
    try {
      const opened = cfg.fs!.open(path, file.signal);
      if (opened instanceof Promise) opened.then(got, failed);
      else got(opened);
    } catch (err) {
      failed(err);
    }
  }

  /** Commits the file written as `fd`: COMMIT's packet is its path, and
   * `bytes` are the whole of what was written. Its close is completed here
   * with the commit's result. */
  function commit_file(fd: u32, f: BranchCursor, bytes: U8Array): void {
    const path = read_path(f.at_branch(0));
    post(
      fd,
      FsCommand.FS_CLOSE,
      frame.RES,
      commit(cfg.fs!, path, [bytes]),
      NOTHING,
    );
  }

  /** Aborts the open of `fd`, which the core closed before it completed. */
  function abort_open(fd: u32): void {
    files.get(fd)?.abort();
    files.delete(fd);
  }

  function hear(
    tag: u32,
    cmd: u32,
    flags: u32,
    res: i32,
    pkt: U8Array | null,
    named: U8Array | null,
  ): void {
    if (flags & frame.RES) {
      const done = waiting.get(tag);
      if (done) {
        waiting.delete(tag);
        return done(res, named);
      }
      if (res < 0) console.warn(`[xpute] 0x${cmd.toString(16)} failed: ${res}`);
      return;
    }
    const f = pkt ? new TreeReader(pkt).read_branch() : null;
    if (cmd === FsCommand.FS_OPEN) {
      return open_file(tag, fs_read[FsCommand.FS_OPEN](f!).path);
    }
    if (cmd === batch.COMMIT) return commit_file(tag, f!, named ?? NOTHING);
    if (cmd === FsCommand.FS_CLOSE) return abort_open(tag);
    const answered = answer(tag, cmd, f, named, pkt);
    if (answered !== undefined && (flags & frame.ACKREQ || answered < 0)) {
      post(tag, cmd, frame.RES, answered, NOTHING);
    }
  }

  function stop(): void {
    if (stopped) return;
    stopped = true;
    waiting.forEach((done) => done(Errno.ENOTRECOVERABLE, null));
    waiting.clear();
    ui.stop();
  }

  XPUTE.callOn((call_at) => {
    const c = frame.frame_at(mem, call_at);
    const f = c.pkt ? new TreeReader(c.pkt).read_branch() : null;
    return answer(c.tag, c.cmd, f, null, c.pkt) ?? 0;
  });
  XPUTE.addListener("onNotify", () => {
    XPUTE.answer();
    const down = XPUTE.take();
    const words = new DataView(down);
    for (let off = 0; off < down.byteLength;) {
      const word = (i: u32) => words.getUint32(off + 4 * i, true);
      const [tag, cmd, flags, res, pkt_len, named_len] = [
        word(batch.TAG),
        word(batch.CMD),
        word(batch.FLAGS),
        words.getInt32(off + 4 * batch.RESULT, true),
        word(batch.PKT),
        word(batch.NAMED),
      ];
      off += 4 * batch.HDR_WORDS;
      const pkt = pkt_len > 0 ? new Uint8Array(down, off, pkt_len) : null;
      off += padded(pkt_len);
      const named = new Uint8Array(down, off, named_len);
      off += padded(named_len);
      if (cmd === batch.STOPPED) stop();
      else hear(tag, cmd, flags, res, pkt, named_len > 0 ? named : null);
    }
  });
  XPUTE.pace(
    pace.pace_hz,
    pace.margin_share,
    pace.batch_frames,
    pace.settle_ms,
  );
  XPUTE.attach(manifest.fs);
  XPUTE.pause();

  // Turns run while resumed and shown.
  let resumed = false;
  let shown = true;
  const steer = () => (resumed && shown ? XPUTE.resume() : XPUTE.pause());

  // The core hears a runtime attach before anything else.
  send(Command.SYS_ATTACH);
  return {
    send,
    request(cmd, ...args) {
      if (stopped) return Promise.reject(new XputeError(Errno.ENOTRECOVERABLE));
      next = (next + 1) >>> 0 || 1;
      const id = next;
      return new Promise<[i32, U8Array | null]>((done) => {
        waiting.set(id, (res, named) => done([res, named]));
        post(id, cmd, frame.ACKREQ, 0, packet(args));
      }).then(reply_of);
    },
    fits: (_cmd, ...args) => (encode_args(args)(room)?.finish() ?? 0) >= 0,
    wake: () => XPUTE.wake(),
    input: () => XPUTE.input(),
    resume: () => ((resumed = true), steer()),
    pause: () => ((resumed = false), steer()),
    shown: () => shown,
    visible(visible) {
      if (visible === shown) return;
      shown = visible;
      // SYS_SUSPEND goes before turns stop, and a pause lets what waits for
      // the core reach it (native/thread.rs `pause`), since hiding is the
      // core's last sure moment to save what it holds.
      send(visible ? Command.SYS_RESUME : Command.SYS_SUSPEND);
      steer();
    },
  };
}

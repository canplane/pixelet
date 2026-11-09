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
import { InvariantError } from "@xpute/kit/status/error.ts";
import {
  type Arg,
  type BranchCursor,
  type PacketRoom,
  PacketWriter,
  TreeReader,
} from "@xpute/kit/wire/xtp/mod.ts";
import { XPUTE } from "@xpute/native";
import { Command as FsCommand, read as fs_read } from "./abi/fs.spec.ts";
import { Command as GpuCommand } from "./abi/gpu.spec.ts";
import { Command, sys_imports } from "./abi/sys.spec.ts";
import { encode_args } from "./ipc/doorbell.ts";
import { FileError } from "./ipc/fs.ts";
import { flag } from "./ipc/fs.spec.ts";
import * as frame from "./ipc/frame.ts";
import { Head, lay, laid } from "./ipc/head.ts";
import { Ring } from "./ipc/ring.ts";
import {
  abort_report,
  bind_mem,
  type Link,
  type Manifest,
  type Ui,
} from "./runtime.ts";
import { FramePacing } from "./sched/frame_pacing.ts";
import type { Grant } from "./sched/policy.ts";

/** The command of the frame that says the core has stopped (native/serve.rs
 * `STOPPED`). */
const STOPPED = 0xffffffff;
/** The little-endian words before a frame's bytes in a batch, and what those
 * bytes are padded to. */
const HEAD_BYTES = 24;
const ALIGN = 8;
/** The UI's ids stay below the one the runtime keeps for itself. */
const ID_CAP = 0x7fffffff;
const NOTHING = new Uint8Array(0);

const padded = (n: u32) => Math.ceil(n / ALIGN) * ALIGN;

export async function start<G extends Grant>(
  m: Manifest<G>,
  ui: Ui,
): Promise<Link> {
  const cfg = await m.core();
  if (m.machine.fs !== (cfg.fs !== undefined)) {
    throw new InvariantError(Errno.EINVAL);
  }
  // A device's runtime paces its turns to the display, by the program's
  // numbers, and runs no other policy.
  if (!(cfg.policy instanceof FramePacing)) {
    throw new InvariantError(Errno.ENOTSUP);
  }
  const pace = cfg.policy.cfg;
  const mem = XPUTE.mem();
  const at = m.machine.head_at;
  if (!laid(mem, at)) lay(mem, at, m.machine);
  bind_mem(mem);
  const head = new Head(mem, at);
  const cq = new Ring(mem, head.cq_at);

  const buf = new Uint8Array(m.machine.slot_bytes);
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

  const waiting = new Map<u32, (res: i32) => void>();
  let next = 0;
  let stopped = false;
  const post = (tag: u32, cmd: u32, flags: u32, res: i32, pkt: U8Array) =>
    XPUTE.post(tag, cmd, flags, res, pkt as Uint8Array);
  const send = (cmd: u32, ...args: Arg[]) => {
    if (!stopped) post(0, cmd, 0, 0, packet(args));
  };

  const own = sys_imports({
    [Command.SYS_ABORT]: (e) => console.error(abort_report(e)),
    // The runtime's thread takes the core's pace (native/serve.rs).
    [Command.SYS_PACE]: () => {},
  });
  const ui_cmds = new Set(ui.cmds);

  /** Answers a frame as whatever takes its major does. */
  function answer(
    tag: u32,
    cmd: u32,
    f: BranchCursor | null,
    named: U8Array | null,
    pkt: U8Array | null,
  ): i32 | void {
    const major = frame.major_of(cmd);
    const on =
      major < frame.PROGRAM_MAJOR
        ? major === 1
          ? own
          : undefined
        : cfg.program;
    const res = on ? on(tag, cmd, f, pkt) : Errno.ENOSYS;
    if (ui_cmds.has(cmd)) ui.on(tag, cmd, f, named, pkt);
    return res;
  }

  interface File {
    root: u32;
    path: string;
    controller: AbortController;
  }
  const files = new Map<u32, File>();

  function open_file(fd: u32, root: u32, path: string, write: boolean): void {
    const file: File = { root, path, controller: new AbortController() };
    files.set(fd, file);
    if (write) return;
    const failed = (err: unknown) => {
      if (files.get(fd) !== file) return;
      files.delete(fd);
      if (!(err instanceof FileError)) {
        console.warn(`[xpute/fs] open ${root}/${path}: ${String(err)}`);
      }
      const [errnum, detail] =
        err instanceof FileError ? [err.errnum, err.detail] : [Errno.EIO, 0];
      XPUTE.fsOpened(fd, errnum, detail, NOTHING);
    };
    const got = (bytes: U8Array) => {
      if (files.get(fd) !== file) return;
      XPUTE.fsOpened(fd, bytes.byteLength, 0, bytes as Uint8Array);
    };
    try {
      const opened = cfg.fs!.open(root, path, file.controller.signal);
      if (opened instanceof Promise) opened.then(got, failed);
      else got(opened);
    } catch (err) {
      failed(err);
    }
  }

  function close_file(fd: u32, commit: boolean, bytes: U8Array | null): void {
    const file = files.get(fd);
    files.delete(fd);
    if (!commit) return file?.controller.abort();
    let res: i32 = Errno.OK;
    try {
      if (file) cfg.fs!.commit(file.root, file.path, bytes ?? NOTHING);
    } catch (err) {
      res = err instanceof FileError ? err.errnum : Errno.EIO;
    }
    XPUTE.fsClosed(fd, res);
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
        return done(res);
      }
      if (res < 0) console.warn(`[xpute] 0x${cmd.toString(16)} failed: ${res}`);
      return;
    }
    const f = pkt ? new TreeReader(pkt).read_branch() : null;
    if (cmd === FsCommand.FS_OPENAT) {
      const e = fs_read[FsCommand.FS_OPENAT](f!);
      return open_file(e.fd, e.root, e.path, e.flags === flag.O_WRONLY);
    }
    if (cmd === FsCommand.FS_CLOSE) return close_file(tag, res === 1, named);
    const answered = answer(tag, cmd, f, named, pkt);
    if (flags & frame.ACKREQ && answered !== undefined) {
      post(tag, cmd, frame.RES, answered, NOTHING);
    }
  }

  function stop(): void {
    if (stopped) return;
    stopped = true;
    waiting.forEach((done) => done(Errno.ENOTRECOVERABLE));
    waiting.clear();
    ui.stop();
  }

  XPUTE.callOn((call_at) => {
    const e = cq.entry_of(call_at);
    const pkt = cq.pkt(e);
    const f = pkt ? new TreeReader(pkt).read_branch() : null;
    return answer(cq.tag(e), cq.cmd(e), f, null, pkt) ?? 0;
  });
  XPUTE.addListener("onNotify", () => {
    XPUTE.answer();
    const batch = XPUTE.take();
    const words = new DataView(batch);
    for (let off = 0; off < batch.byteLength;) {
      const word = (i: u32) => words.getUint32(off + 4 * i, true);
      const [tag, cmd, flags, res, pkt_len, named_len] = [
        word(0),
        word(1),
        word(2),
        words.getInt32(off + 12, true),
        word(4),
        word(5),
      ];
      off += HEAD_BYTES;
      const pkt = pkt_len > 0 ? new Uint8Array(batch, off, pkt_len) : null;
      off += padded(pkt_len);
      const named = new Uint8Array(batch, off, named_len);
      off += padded(named_len);
      if (cmd === STOPPED) stop();
      else
        hear(
          tag,
          cmd,
          flags,
          res,
          pkt,
          named_len > 0 || res === 1 ? named : null,
        );
    }
  });
  XPUTE.pace(
    pace.pace_hz,
    pace.margin_share,
    pace.batch_frames,
    pace.settle_ms,
  );
  XPUTE.attach(head.sq_at, head.cq_at, m.machine.fs);
  XPUTE.pause();

  // Turns run while resumed and shown.
  let resumed = false;
  let shown = true;
  const steer = () => (resumed && shown ? XPUTE.resume() : XPUTE.pause());

  const gpu = m.machine.gpu ? cfg.gpu : undefined;
  if (gpu) {
    gpu.watch(ui.gpu);
    gpu.grant({
      mem,
      at: head.gpu_at,
      ask: m.machine.gpu!,
      ready: () => send(GpuCommand.GPU_READY),
      wake: () => XPUTE.wake(),
      shown: () => shown,
    });
  }

  // The core hears a runtime attach before anything else.
  send(Command.SYS_ATTACH);
  return {
    send,
    request(cmd, ...args) {
      if (stopped) return Promise.resolve(Errno.ENOTRECOVERABLE);
      next = (next % ID_CAP) + 1;
      const id = next;
      return new Promise((done) => {
        waiting.set(id, done);
        post(id, cmd, frame.ACKREQ, 0, packet(args));
      });
    },
    call(cmd, ...args) {
      if (stopped) return Errno.ENOTRECOVERABLE;
      const res = XPUTE.call(cmd, packet(args) as Uint8Array);
      if (res < 0) console.warn(`[xpute] 0x${cmd.toString(16)}: ${res}`);
      return res;
    },
    fits: (_cmd, ...args) => (encode_args(args)(room)?.finish() ?? 0) >= 0,
    wake: () => XPUTE.wake(),
    input: () => XPUTE.input(),
    resume: () => ((resumed = true), steer()),
    pause: () => ((resumed = false), steer()),
    visible(visible) {
      if (visible === shown) return;
      shown = visible;
      steer();
      // SYS_SUSPEND and SYS_RESUME go as a call, because hiding is the core's
      // last sure moment to save what it holds.
      XPUTE.call(visible ? Command.SYS_RESUME : Command.SYS_SUSPEND, NOTHING);
    },
    bind(): never {
      throw new InvariantError(Errno.ENOTSUP);
    },
    unbind(): never {
      throw new InvariantError(Errno.ENOTSUP);
    },
  };
}

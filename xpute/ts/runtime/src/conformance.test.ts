// @xpute/runtime/conformance.test.ts

/**
 * This runs the traces under spec/conformance/ against every core, Rust or C++,
 * native or WebAssembly, through this package's host, header and stream,
 * under each of the layouts a core draws from its seed (LAYOUTS). Build the
 * cores first: the Rust one with Cargo, natively and for wasm32, and the C++
 * one with cpp/Makefile.
 */

import { test } from "vitest";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import koffi from "koffi";

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { Golden } from "../../dev/golden.ts";
import { PacketWriter } from "@xpute/kit/wire/xtp/mod.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { Command } from "@xpute/runtime/abi/sys.spec.ts";
import * as fs from "@xpute/runtime/abi/fs.spec.ts";
import { handle_slot } from "@xpute/runtime/abi/handle.ts";
import { Head } from "@xpute/runtime/io/head.ts";
import { WORDS, word } from "@xpute/runtime/io/head.spec.ts";
import { UNSET } from "@xpute/runtime/io/connection.ts";
import * as frame from "@xpute/runtime/io/frame.ts";
import { each_rec } from "@xpute/runtime/io/stream.ts";
import { open_module } from "@xpute/runtime/module.ts";
import {
  cmd,
  LAYOUTS,
  MEM_BYTES,
  PKT_BYTES,
} from "@xpute/runtime/conformance.spec.ts";
import type { Core } from "@xpute/runtime/runtime.ts";
import { TreeReader } from "@xpute/kit/wire/xtp/mod.ts";

const ROOT = fileURLToPath(new URL("../../../", import.meta.url));
/** A relative CARGO_TARGET_DIR is relative to rust/, where the root's tasks
 * run cargo. */
const CARGO_TARGET = resolve(
  join(ROOT, "rust"),
  process.env.CARGO_TARGET_DIR ?? "target",
);

function library(dir: string, name: string): string {
  return join(
    dir,
    process.platform === "darwin"
      ? `lib${name}.dylib`
      : process.platform === "win32"
        ? `${name}.dll`
        : `lib${name}.so`,
  );
}

const NOW = koffi.proto("double xpute_now(void)");
const RECEIVE = koffi.proto("int32_t xpute_receive(uint32_t at, uint32_t cap)");
const SEND = koffi.proto(
  "void xpute_send(uint32_t at, uint32_t named_at, uint32_t named_len)",
);
const CALL = koffi.proto("int32_t xpute_call(uint32_t at)");

/** A module's whole memory: the core's range, and room past it for the
 * module's own stack and data, whose sizes the build decides. */
const MODULE_BYTES = 4 << 20;

const MODULE_IMPORTS = [
  "env.call",
  "env.memory",
  "env.now",
  "env.receive",
  "env.send",
];

interface Opener {
  /** `open` boots a core under the layout `seed` draws, and answers it with
   * the offset its boot answered for the front's header. */
  open(seed: number): [Core, number];
  close(): void;
}

function native(path: string): Opener {
  const lib = koffi.load(path);
  const boot = lib.func(
    "uint32_t xpute_conformance_boot(void *mem, xpute_now *now, xpute_receive *receive, xpute_send *send, xpute_call *call, uint32_t seed)",
  );
  const core_run = lib.func(
    "double xpute_conformance_run(void *mem, double deadline)",
  );
  const now = koffi.register(() => clock_ms, koffi.pointer(NOW));
  let host = UNSET;
  const receive = koffi.register(
    (at: number, cap: number) => host.receive(at, cap),
    koffi.pointer(RECEIVE),
  );
  const send = koffi.register(
    (at: number, named_at: number, named_len: number) =>
      host.send(at, named_at, named_len),
    koffi.pointer(SEND),
  );
  const call = koffi.register(
    (at: number) => host.call(at),
    koffi.pointer(CALL),
  );
  // The core keeps the region between calls, so it must be memory that does
  // not move and is seen from here without a copy. It is allocated as doubles
  // so that it is 8-aligned, as a core requires.
  const regions: unknown[] = [];
  return {
    open(seed) {
      const at = koffi.alloc("double", MEM_BYTES / 8);
      regions.push(at);
      host = UNSET;
      const head_at = boot(at, now, receive, send, call, seed);
      const core: Core = {
        mem: koffi.view(at, MEM_BYTES),
        run: (deadline) => core_run(at, deadline),
        host_with: (h) => (host = h),
      };
      return [core, head_at];
    },
    close() {
      for (const f of [now, receive, send, call]) koffi.unregister(f);
      lib.unload();
      for (const at of regions) koffi.free(at);
    },
  };
}

function module(path: string): Opener {
  const compiled = new WebAssembly.Module(readFileSync(path));
  const imports = WebAssembly.Module.imports(compiled)
    .map((i) => `${i.module}.${i.name}`)
    .sort();
  assert.deepStrictEqual(
    imports,
    MODULE_IMPORTS,
    `${path}: what the module reaches its runtime through`,
  );
  return {
    open(seed) {
      const pages = MODULE_BYTES >> 16;
      const mem = new WebAssembly.Memory({ initial: pages, maximum: pages });
      const core = open_module(compiled, mem, () => clock_ms);
      const boot = core.instance.exports.xpute_conformance_boot as (
        seed: number,
      ) => number;
      return [core, boot(seed) >>> 0];
    },
    close() {},
  };
}

const CORES: Record<string, () => Opener> = {
  "rust native": () =>
    native(library(join(CARGO_TARGET, "debug"), "xpute_conformance")),
  "cpp native": () =>
    native(library(join(ROOT, "cpp/target"), "xpute_conformance")),
  "rust wasm": () =>
    module(
      join(CARGO_TARGET, "wasm32-unknown-unknown/debug/xpute_conformance.wasm"),
    ),
  "cpp wasm": () => module(join(ROOT, "cpp/target/xpute_conformance.wasm")),
};

const MOST_SETTLE_TURNS = 64;

let clock_ms = 0;

interface Completion {
  tag: number;
  cmd: number;
  reply: boolean;
  result: number;
  /** The memory the frame named, as it was when sent. */
  named: U8Array;
}

interface Turn {
  completions: Completion[];
  stream: number[];
  wake: number;
}

class Runtime {
  readonly head: Uint32Array;
  private readonly _stream_at: number;
  private readonly _writer = new PacketWriter();
  private readonly _room: U8Array = new Uint8Array(new ArrayBuffer(PKT_BYTES));
  /** What waits for the core, and what it sent this turn. */
  private readonly _waiting: frame.Frame[] = [];
  private _sent: (frame.Frame & { named: U8Array })[] = [];

  private readonly _core: Core;

  constructor(core: Core, head_at: number) {
    this._core = core;
    const mem = core.mem;
    const head = new Head(mem, head_at);
    this.head = new Uint32Array(mem, head_at, WORDS);
    this._stream_at = head.stream_at;
    core.host_with({
      receive: (at, cap) => {
        const f = this._waiting.shift();
        if (f === undefined) return 0;
        assert.ok(
          frame.WORD_BYTES + (f.pkt?.byteLength ?? 0) <= cap,
          "a frame that fits the core's room",
        );
        frame.lay_frame(mem, at, f);
        return 1;
      },
      send: (at, named_at, named_len) => {
        const f = frame.frame_at(mem, at);
        const named = new Uint8Array(mem, named_at, named_len).slice();
        this._sent.push({ ...f, pkt: f.pkt && f.pkt.slice(), named });
      },
      // The fixture makes one call, INC, as a frame it laid at `at`.
      call: (at) => {
        const c = frame.frame_at(mem, at);
        if (c.cmd !== cmd.INC) return Errno.ENOSYS;
        if (c.pkt === null) return Errno.EBADMSG;
        const f = new TreeReader(c.pkt).read_branch();
        const [n, word] = [f.at(0).get<number>(), f.at(1).get<number>()];
        if (n === 0) return Errno.EINVAL;
        new Uint32Array(mem, word, 1)[0] = n + 1;
        return 0;
      },
    });
  }

  submit(tag: number, cmd: number, ack: boolean, args: number[]): void {
    this._push(tag, cmd, ack ? frame.ACKREQ : 0, 0, args);
  }

  complete(tag: number, cmd: number, result: number, args: number[]): void {
    this._push(tag, cmd, frame.RES, result, args);
  }

  private _push(
    tag: number,
    cmd: number,
    flags: number,
    result: number,
    args: number[],
  ): void {
    let pkt: U8Array | null = null;
    if (args.length > 0) {
      const w = this._writer.begin(
        this._room,
        new DataView(this._room.buffer),
        0,
        this._room.byteLength,
        args.length,
      );
      // A negative number, such as an open's -errno, is written as an i32.
      for (const a of args) {
        if (a < 0) w.i32(a);
        else w.u32(a);
      }
      const end = w.finish();
      assert.ok(end >= 0, "a packet that fits a frame");
      pkt = this._room.slice(0, end);
    }
    this._waiting.push({ tag, cmd, flags, result, pkt });
  }

  run(deadline_ms: number): Turn {
    const wake = this._core.run(deadline_ms);
    const stream: number[] = [];
    each_rec(this._core.mem, this._stream_at, (op) => stream.push(op));
    const sent = this._sent;
    this._sent = [];
    const completions: Completion[] = sent
      // An event of xpute's own, from the runtime's control or the file
      // device, is skipped: no trace's runtime takes one.
      .filter(
        (f) =>
          frame.CMD_MAJOR(f.cmd) >= frame.PROGRAM_MAJOR ||
          (f.flags & frame.RES) !== 0,
      )
      .map((f) => ({
        tag: f.tag,
        cmd: f.cmd,
        reply: (f.flags & frame.RES) !== 0,
        result: f.result,
        named: f.named,
      }));
    return { completions, stream, wake };
  }

  get pending(): boolean {
    return this._waiting.length > 0;
  }

  word(name: string): number {
    if (!(name in word)) throw new Error(`no header word ${name}`);
    return this.head[word[name as keyof typeof word]];
  }
}

/** These are the core's own commands, by name. */
const CMD_NAMES = Object.entries(cmd).filter(
  ([, v]) => typeof v === "number",
) as [string, number][];

/** These are the commands of xpute's own that a trace sends or completes: the
 * runtime's control commands and the file device's. */
const RUNTIME_NAMES: [string, number][] = [
  ["NOP", Command.NOP],
  ["FS_OPEN", fs.Command.FS_OPEN],
  ["FS_CLOSE", fs.Command.FS_CLOSE],
];

/** The fixture's numbers, which the header's size words must repeat. */
const SIZES: Record<string, number> = { PKT_BYTES };

/** Reads a command named in either table, or written as a number in hex. */
function cmd_number(name: string): number {
  const found = [...CMD_NAMES, ...RUNTIME_NAMES].find(([n]) => n === name);
  if (found) return found[1];
  const n = parseInt(name.replace(/^0x/, ""), 16);
  if (Number.isNaN(n)) throw new Error(`no command ${name}`);
  return n;
}

function cmd_name(n: number): string {
  return (
    [...RUNTIME_NAMES, ...CMD_NAMES].find(([, v]) => v === n)?.[0] ??
    `0x${n.toString(16).padStart(4, "0")}`
  );
}

/** Renders what the core sent as a trace writes it: a reply as
 * TAG:CMD=RESULT, and the memory it named after it in hex, as
 * TAG:CMD=RESULT[0102]; an event as TAG:CMD, and a run of one command's
 * events tagged 0, 1, 2 and up as CMD*n. */
function render_completions(completions: Completion[]): string {
  const out: string[] = [];
  let run: [number, number] | null = null;
  const flush = () => {
    if (run) out.push(`${cmd_name(run[0])}*${run[1]}`);
    run = null;
  };
  for (const c of completions) {
    if (!c.reply) {
      if (run && run[0] === c.cmd && c.tag === run[1]) {
        run = [run[0], run[1] + 1];
      } else {
        flush();
        if (c.tag === 0) run = [c.cmd, 1];
        else out.push(`${c.tag}:${cmd_name(c.cmd)}`);
      }
      continue;
    }
    flush();
    const named = [...c.named]
      .map((b) => b.toString(16).padStart(2, "0"))
      .join("");
    out.push(`${c.tag}:${cmd_name(c.cmd)}=${c.result}${named && `[${named}]`}`);
  }
  flush();
  return out.length === 0 ? "-" : out.join(" ");
}

function render_stream(ops: number[]): string {
  return ops.length === 0 ? "-" : ops.join(" ");
}

/** Matches what was seen against what the trace says. A result written `$n`
 * binds that variable the first time it is seen, and must match it every
 * time after. */
function expect(
  seen: string,
  said: string,
  vars: Map<string, number>,
  what: string,
): void {
  const seen_tokens = seen.split(" ");
  const said_tokens = said.split(" ");
  assert.deepStrictEqual(
    seen_tokens.length,
    said_tokens.length,
    `${what}: saw ${seen}, the trace says ${said}`,
  );
  seen_tokens.forEach((seen_token, i) => {
    const said_token = said_tokens[i];
    const [seen_head, val] = seen_token.split("=");
    const [said_head, v] = said_token.split("=");
    if (v !== undefined && v.startsWith("$") && val !== undefined) {
      assert.deepStrictEqual(
        seen_head,
        said_head,
        `${what}: saw ${seen}, the trace says ${said}`,
      );
      if (!vars.has(v)) vars.set(v, Number(val));
      assert.deepStrictEqual(
        vars.get(v),
        Number(val),
        `${what}: ${v} again, and not what it was`,
      );
    } else {
      assert.deepStrictEqual(
        seen_token,
        said_token,
        `${what}: saw ${seen}, the trace says ${said}`,
      );
    }
  });
}

/** Reads a number a trace writes: a literal, `$n`, or `slot $n`, the slot
 * that the handle bound to `$n` names. */
function val(term: string, vars: Map<string, number>): number {
  const bound = (v: string) => {
    const n = vars.get(v);
    if (n === undefined) throw new Error(`${v} is bound by nothing before it`);
    return n;
  };
  const [head, rest] = term.split(" ");
  if (head === "slot" && rest !== undefined) return handle_slot(bound(rest));
  return term.startsWith("$") ? bound(term) : Number(term);
}

/** Checks a relation the trace says holds between numbers seen: `A == B` or
 * `A != B`. */
function relation(said: string, vars: Map<string, number>, what: string): void {
  if (said.includes(" == ")) {
    const [a, b] = said.split(" == ");
    assert.deepStrictEqual(val(a, vars), val(b, vars), `${what}: ${said}`);
  } else if (said.includes(" != ")) {
    const [a, b] = said.split(" != ");
    assert.notDeepStrictEqual(val(a, vars), val(b, vars), `${what}: ${said}`);
  } else {
    throw new Error(`${what}: no relation in ${said}`);
  }
}

/** Any negative wake means none, as the contract says (ARCHITECTURE.md, the
 * turn); each language's `NO_WAKE` is its own choice of a negative number. */
function wake_name(wake: number): string {
  return wake === 0 ? "next" : wake > 0 ? "later" : "none";
}

function run(
  g: Golden,
  core: string,
  opener: Opener,
  trace: string,
  seed: number,
): void {
  const name = `${core}, layout ${seed}: ${trace}`;
  clock_ms = 0;
  const runtime = new Runtime(...opener.open(seed));
  const turns: Turn[] = [];
  const vars = new Map<string, number>();
  for (let i = 0; g.has(`${trace}.step.${i}`); i++) {
    const step = g.s(`${trace}.step.${i}`).split(" ");
    switch (step[0]) {
      case "submit": {
        runtime.submit(
          Number(step[1]),
          cmd_number(step[2]),
          step[3] === "ack",
          step.slice(4).map((t) => val(t, vars)),
        );
        break;
      }
      case "complete": {
        runtime.complete(
          val(step[1], vars),
          cmd_number(step[2]),
          val(step[3], vars),
          step.slice(4).map((t) => val(t, vars)),
        );
        break;
      }
      case "run": {
        // A turn is held to the trace as soon as it is seen, so the results
        // it binds can be named by the steps after it.
        const turn = runtime.run(Number(step[1]));
        const at = `${trace}.turn.${turns.length}.completions`;
        if (g.has(at)) {
          expect(
            render_completions(turn.completions),
            g.s(at),
            vars,
            `${name} turn ${turns.length}`,
          );
        }
        turns.push(turn);
        break;
      }
      case "clock":
        clock_ms = Number(step[1]);
        break;
      case "settle":
        for (;;) {
          turns.push(runtime.run(0));
          if (!runtime.pending) break;
          assert.ok(
            turns.length < MOST_SETTLE_TURNS,
            `${name}: the core stopped applying what waits`,
          );
        }
        break;
      default:
        throw new Error(`${name}: no step ${step.join(" ")}`);
    }
  }
  turns.forEach((turn, t) => {
    const at = (what: string) => `${trace}.turn.${t}.${what}`;
    if (g.has(at("stream"))) {
      assert.deepStrictEqual(
        render_stream(turn.stream),
        g.s(at("stream")),
        `${name} turn ${t}: the stream`,
      );
    }
    if (g.has(at("wake"))) {
      assert.deepStrictEqual(
        wake_name(turn.wake),
        g.s(at("wake")),
        `${name} turn ${t}: what it asked for next`,
      );
    }
  });
  const at = (what: string) => `${trace}.${what}`;
  if (g.has(at("completions"))) {
    assert.deepStrictEqual(
      render_completions(turns.flatMap((t) => t.completions)),
      g.s(at("completions")),
      `${name}: what the core sent`,
    );
  }
  if (g.has(at("stream"))) {
    assert.deepStrictEqual(
      render_stream(turns.flatMap((t) => t.stream)),
      g.s(at("stream")),
      `${name}: the stream`,
    );
  }
  if (g.has(at("turns.least"))) {
    assert.ok(
      turns.length >= g.u32(at("turns.least")),
      `${name}: ${turns.length} turns`,
    );
  }
  if (g.has(at("wake.before_last"))) {
    turns
      .slice(0, -1)
      .forEach((turn, t) =>
        assert.deepStrictEqual(
          wake_name(turn.wake),
          g.s(at("wake.before_last")),
          `${name} turn ${t}: what it asked for next`,
        ),
      );
  }
  if (g.has(at("wake.last"))) {
    assert.deepStrictEqual(
      wake_name(turns[turns.length - 1].wake),
      g.s(at("wake.last")),
      `${name}: what the last turn asked for`,
    );
  }
  for (let r = 0; g.has(at(`relation.${r}`)); r++) {
    relation(g.s(at(`relation.${r}`)), vars, name);
  }
  if (g.has(at("head.sizes"))) {
    for (const k of g.s(at("head.sizes")).split(" ")) {
      assert.deepStrictEqual(
        runtime.word(k),
        SIZES[k],
        `${name}: the header's ${k}`,
      );
    }
  }
  if (g.has(at("head.places"))) {
    for (const k of g.s(at("head.places")).split(" ")) {
      assert.notDeepStrictEqual(
        runtime.word(k),
        0,
        `${name}: the header's ${k}`,
      );
    }
  }
}

for (const trace of [
  "head",
  "door",
  "handle",
  "clock",
  "file",
  "call",
  "reply",
]) {
  for (const [core, open] of Object.entries(CORES)) {
    test(`trace ${trace} - the ${core} core`, async () => {
      const g = await Golden.load(`conformance/${trace}.tsv`);
      const opener = open();
      try {
        for (let seed = 0; seed < LAYOUTS; seed++) {
          for (const scenario of g.s("scenarios").split(" ")) {
            run(g, core, opener, scenario, seed);
          }
        }
      } finally {
        opener.close();
      }
    });
  }
}

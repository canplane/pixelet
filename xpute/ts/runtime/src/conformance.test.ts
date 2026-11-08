// @xpute/runtime/conformance.test.ts

/**
 * This runs the traces under spec/conformance/ against every core, Rust or C++,
 * native or WebAssembly, through this package's rings, header and stream, under
 * each of the layouts a core draws from its seed (LAYOUTS). Build the cores
 * first with `make build` at xpute's root, which builds Rust with Cargo and C++
 * with cpp/Makefile.
 */

import { test } from "vitest";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import koffi from "koffi";

import { Golden } from "@xpute/kit/golden.ts";
import { PacketWriter } from "@xpute/kit/wire/xtp/mod.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { Command } from "@xpute/runtime/abi/sys.spec.ts";
import * as fs from "@xpute/runtime/abi/fs.spec.ts";
import { handle_slot } from "@xpute/runtime/abi/handle.ts";
import { Head } from "@xpute/runtime/ipc/head.ts";
import { WORDS, word } from "@xpute/runtime/ipc/head.spec.ts";
import * as frame from "@xpute/runtime/ipc/frame.ts";
import { Ring } from "@xpute/runtime/ipc/ring.ts";
import { each_record } from "@xpute/runtime/ipc/stream.ts";
import { open_module } from "@xpute/runtime/module.ts";
import {
  CQ_CAP,
  cmd,
  LAYOUTS,
  MEM_BYTES,
  SLOT_BYTES,
  SQ_CAP,
} from "@xpute/runtime/conformance.spec.ts";
import * as ring from "@xpute/runtime/ipc/ring.spec.ts";
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
const CALL = koffi.proto("int32_t xpute_call(uint32_t at)");

/** A module's whole memory: the core's range, and room past it for the
 * module's own stack and data, whose sizes the build decides. */
const MODULE_BYTES = 4 << 20;

const MODULE_IMPORTS = ["env.call", "env.memory", "env.now"];

/** This is a core as the runtime holds it, with `head_at`, the offset its boot
 * answered for the front's header. */
interface Opened extends Core {
  head_at: number;
}

interface Opener {
  /** `open` boots a core under the layout `seed` draws. */
  open(seed: number): Opened;
  close(): void;
}

/** This answers a core's calls until its runtime sets an answer. */
function answer_enosys(): number {
  return Errno.ENOSYS;
}

function native(path: string): Opener {
  const lib = koffi.load(path);
  const boot = lib.func(
    "uint32_t xpute_conformance_boot(void *mem, xpute_now *now, xpute_call *call, uint32_t seed)",
  );
  const core_run = lib.func(
    "double xpute_conformance_run(void *mem, double deadline)",
  );
  const now = koffi.register(() => clock_ms, koffi.pointer(NOW));
  let answer: (at: number) => number = answer_enosys;
  const runtime = koffi.register(
    (at: number) => answer(at),
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
      answer = answer_enosys;
      const head_at = boot(at, now, runtime, seed);
      return {
        mem: koffi.view(at, MEM_BYTES),
        head_at,
        run: (deadline) => core_run(at, deadline),
        answer_with: (a) => (answer = a),
      };
    },
    close() {
      koffi.unregister(now);
      koffi.unregister(runtime);
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
      const memory = new WebAssembly.Memory({ initial: pages, maximum: pages });
      const core = open_module(compiled, memory, () => clock_ms);
      const boot = core.instance.exports.xpute_conformance_boot as (
        seed: number,
      ) => number;
      return { ...core, head_at: boot(seed) >>> 0 };
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
}

interface Turn {
  completions: Completion[];
  stream: number[];
  wake: number;
}

class Runtime {
  readonly head: Uint32Array;
  readonly sq: Ring;
  private readonly _cq: Ring;
  private readonly _stream_at: number;
  private readonly _writer = new PacketWriter();

  private readonly _core: Opened;

  constructor(core: Opened) {
    this._core = core;
    const mem = core.mem;
    const head = new Head(mem, core.head_at);
    this.head = new Uint32Array(mem, core.head_at, WORDS);
    this.sq = new Ring(mem, head.sq_at);
    this._cq = new Ring(mem, head.cq_at);
    this._stream_at = head.gpu_at;
    // The fixture makes one call, INC, as a frame it laid at `at` in its
    // completion ring without publishing it.
    core.answer_with((at) => {
      const cq = this._cq;
      const e = cq.entry_of(at);
      if (cq.cmd(e) !== cmd.INC) return Errno.ENOSYS;
      const pkt = cq.pkt(e);
      if (pkt === null) return Errno.EBADMSG;
      const f = new TreeReader(pkt).read_branch();
      const [n, word] = [f.at(0).get<number>(), f.at(1).get<number>()];
      if (n === 0) return Errno.EINVAL;
      new Uint32Array(mem, word, 1)[0] = n + 1;
      return 0;
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
    const sq = this.sq;
    let pkt: PacketWriter | null = null;
    if (args.length > 0) {
      assert.ok(
        sq.write(this._writer, args.length),
        "a full ring takes no packet",
      );
      // A negative number, such as an open's -errno, is written as an i32.
      for (const a of args) {
        if (a < 0) this._writer.i32(a);
        else this._writer.u32(a);
      }
      pkt = this._writer;
    }
    assert.deepStrictEqual(sq.push(tag, cmd, flags, pkt, result), Errno.OK);
  }

  ring(deadline_ms: number): Turn {
    const wake = this._core.run(deadline_ms);
    const stream: number[] = [];
    each_record(this._core.mem, this._stream_at, (op) => stream.push(op));
    const completions: Completion[] = [];
    const cq = this._cq;
    for (let at = cq.peek(); at >= 0; at = cq.peek()) {
      // An event of xpute's own, from the runtime's control or the file
      // device, is skipped: no trace's runtime takes one.
      if (
        frame.major_of(cq.cmd(at)) < frame.PROGRAM_MAJOR &&
        (cq.flags(at) & frame.RES) === 0
      ) {
        cq.advance();
        continue;
      }
      completions.push({
        tag: cq.tag(at),
        cmd: cq.cmd(at),
        reply: (cq.flags(at) & frame.RES) !== 0,
        result: cq.result(at),
      });
      cq.advance();
    }
    return { completions, stream, wake };
  }

  get pending(): boolean {
    return this.sq.len > 0;
  }

  word(name: string): number {
    if (!(name in word)) throw new Error(`no header word ${name}`);
    return this.head[word[name as keyof typeof word]];
  }

  /** This reads word `at` of the own header of the ring that the header word
   * `ring_at` places. */
  ring_word(ring_at: string, at: number): number {
    return new Uint32Array(
      this.head.buffer,
      this.word(ring_at),
      ring.HEADER_WORDS,
    )[at];
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
  ["FS_OPENAT", fs.Command.FS_OPENAT],
  ["FS_CLOSE", fs.Command.FS_CLOSE],
];

/** The fixture's numbers, which the header's size words must repeat. */
const SIZES: Record<string, number> = { SQ_CAP, CQ_CAP, SLOT_BYTES };

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

/** Renders a completion ring's sequence as a trace writes it: a reply as
 * TAG:CMD=RESULT, an event as TAG:CMD, and a run of one command's events
 * tagged 0, 1, 2 and up as CMD*n. */
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
    out.push(`${c.tag}:${cmd_name(c.cmd)}=${c.result}`);
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
  const runtime = new Runtime(opener.open(seed));
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
      case "ring": {
        // A turn is held to the trace as soon as it is seen, so the results
        // it binds can be named by the steps after it.
        const turn = runtime.ring(Number(step[1]));
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
          turns.push(runtime.ring(0));
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
      `${name}: the completion ring`,
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
    assert.deepStrictEqual(
      [
        runtime.ring_word("SQ_AT", ring.CAP),
        runtime.ring_word("CQ_AT", ring.CAP),
        runtime.ring_word("SQ_AT", ring.SLOT_BYTES),
        runtime.ring_word("CQ_AT", ring.SLOT_BYTES),
      ],
      [SQ_CAP, CQ_CAP, SLOT_BYTES, SLOT_BYTES],
      `${name}: the rings' own headers`,
    );
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

for (const trace of ["head", "door", "handle", "clock", "file", "call"]) {
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

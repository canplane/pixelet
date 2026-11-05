// @xpute/host/conformance.test.ts

/**
 * Runs the traces under spec/conformance/ against every guest, Rust or
 * C++, native or WebAssembly, through this runtime's host, under each of the
 * layouts a guest draws from its seed (LAYOUTS). Build the guests first
 * (`make build` at xpute's root): Cargo for Rust, cpp/Makefile for C++.
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
import { Command } from "@xpute/host/abi/cmd.spec.ts";
import * as fs from "@xpute/host/abi/fs.spec.ts";
import { handle_slot } from "@xpute/host/abi/handle.ts";
import { Head } from "@xpute/host/ipc/head.ts";
import { WORDS, word } from "@xpute/host/ipc/head.spec.ts";
import * as frame from "@xpute/host/ipc/frame.ts";
import { Ring } from "@xpute/host/ipc/ring.ts";
import { each_record } from "@xpute/host/ipc/stream.ts";
import { open_module } from "@xpute/host/module.ts";
import {
  CQ_CAP,
  cmd,
  LAYOUTS,
  MEM_BYTES,
  SLOT_BYTES,
  SQ_CAP,
} from "@xpute/host/conformance.spec.ts";
import * as ring from "@xpute/host/ipc/ring.spec.ts";
import type { Guest } from "@xpute/host/host.ts";
import { TreeReader } from "@xpute/kit/wire/xtp/mod.ts";

const ROOT = fileURLToPath(new URL("../../../", import.meta.url));
/** A relative CARGO_TARGET_DIR is cargo's, which the root's tasks run from rust/. */
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
const HOSTCALL = koffi.proto("int32_t xpute_hostcall(uint32_t at)");

/** Room past the guest's range for the module's own stack and data, which the build decides. */
const MODULE_BYTES = 4 << 20;

const MODULE_IMPORTS = ["env.hostcall", "env.memory", "env.now"];

/** A guest as the host holds it, and where its boot answered its front's
 * header is. */
interface Opened extends Guest {
  head_at: number;
}

interface Opener {
  /** A guest booted under the layout `seed` draws. */
  open(seed: number): Opened;
  close(): void;
}

/** What answers a guest's hostcalls until its host says. */
function answer_enosys(): number {
  return Errno.ENOSYS;
}

function native(path: string): Opener {
  const lib = koffi.load(path);
  const boot = lib.func(
    "uint32_t xpute_conformance_boot(void *mem, xpute_now *now, xpute_hostcall *hostcall, uint32_t seed)",
  );
  const guest_run = lib.func(
    "double xpute_conformance_run(void *mem, double quota)",
  );
  const now = koffi.register(() => clock_ms, koffi.pointer(NOW));
  let answer: (at: number) => number = answer_enosys;
  const host = koffi.register(
    (at: number) => answer(at),
    koffi.pointer(HOSTCALL),
  );
  // The guest keeps the region between calls, so it is memory that does not
  // move, seen from here without a copy. Words of eight bytes, so it is
  // 8-aligned, as a guest asks.
  const regions: unknown[] = [];
  return {
    open(seed) {
      const at = koffi.alloc("double", MEM_BYTES / 8);
      regions.push(at);
      answer = answer_enosys;
      const head_at = boot(at, now, host, seed);
      return {
        mem: koffi.view(at, MEM_BYTES),
        head_at,
        run: (quota) => guest_run(at, quota),
        answer_with: (a) => (answer = a),
      };
    },
    close() {
      koffi.unregister(now);
      koffi.unregister(host);
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
    `${path}: what the module reaches its host through`,
  );
  return {
    open(seed) {
      const pages = MODULE_BYTES >> 16;
      const memory = new WebAssembly.Memory({ initial: pages, maximum: pages });
      const guest = open_module(compiled, memory, () => clock_ms);
      const boot = guest.instance.exports.xpute_conformance_boot as (
        seed: number,
      ) => number;
      return { ...guest, head_at: boot(seed) >>> 0 };
    },
    close() {},
  };
}

const GUESTS: Record<string, () => Opener> = {
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

class Host {
  /** The header's words. */
  readonly head: Uint32Array;
  readonly sq: Ring;
  private readonly cq: Ring;
  private readonly stream_at: number;
  private readonly writer = new PacketWriter();

  private readonly guest: Opened;

  constructor(guest: Opened) {
    this.guest = guest;
    const mem = guest.mem;
    const head = new Head(mem, guest.head_at);
    this.head = new Uint32Array(mem, guest.head_at, WORDS);
    this.sq = new Ring(mem, head.sq_at);
    this.cq = new Ring(mem, head.cq_at);
    this.stream_at = head.gpu_at;
    // The fixture's one call, INC: the frame the guest laid unpublished at
    // `at` in its completion ring.
    guest.answer_with((at) => {
      const cq = this.cq;
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
    const sq = this.sq;
    let pkt: PacketWriter | null = null;
    if (args.length > 0) {
      assert.ok(
        sq.write(this.writer, args.length),
        "a full ring takes no packet",
      );
      // A negative number, such as an open's -errno, as the i32 it is.
      for (const a of args) {
        if (a < 0) this.writer.i32(a);
        else this.writer.u32(a);
      }
      pkt = this.writer;
    }
    assert.deepStrictEqual(
      sq.push(tag, cmd, ack ? frame.ACKREQ : 0, pkt),
      Errno.OK,
    );
  }

  ring(quota_ms: number): Turn {
    const wake = this.guest.run(quota_ms);
    const stream: number[] = [];
    each_record(this.guest.mem, this.stream_at, (op) => stream.push(op));
    const completions: Completion[] = [];
    const cq = this.cq;
    for (let at = cq.peek(); at >= 0; at = cq.peek()) {
      // An event of xpute's own, the host's control or the file device's,
      // which no trace's host takes.
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

  /** Whether a submission waits. */
  get pending(): boolean {
    return this.sq.len > 0;
  }

  /** The header's word `name` says. */
  word(name: string): number {
    if (!(name in word)) throw new Error(`no header word ${name}`);
    return this.head[word[name as keyof typeof word]];
  }

  /** The ring header's word `at` of the ring the header's `ring_at` names. */
  ring_word(ring_at: string, at: number): number {
    return new Uint32Array(
      this.head.buffer,
      this.word(ring_at),
      ring.HEADER_WORDS,
    )[at];
  }
}

/** The guest's own commands, by name. */
const CMD_NAMES = Object.entries(cmd).filter(
  ([, v]) => typeof v === "number",
) as [string, number][];

/** xpute's own a trace sends: the host's control's and the file device's. */
const HOST_NAMES: [string, number][] = [
  ["NOP", Command.NOP],
  ["FS_OPENED", fs.Command.FS_OPENED],
];

/** The fixture's numbers a header's size words say again. */
const SIZES: Record<string, number> = { SQ_CAP, CQ_CAP, SLOT_BYTES };

/** A name of either table, or a number in hex. */
function cmd_number(name: string): number {
  const found = [...CMD_NAMES, ...HOST_NAMES].find(([n]) => n === name);
  if (found) return found[1];
  const n = parseInt(name.replace(/^0x/, ""), 16);
  if (Number.isNaN(n)) throw new Error(`no command ${name}`);
  return n;
}

function cmd_name(n: number): string {
  return (
    [...HOST_NAMES, ...CMD_NAMES].find(([, v]) => v === n)?.[0] ??
    `0x${n.toString(16).padStart(4, "0")}`
  );
}

/** A completion ring's sequence as a trace writes it: a reply TAG:CMD=RESULT,
 * an event TAG:CMD, and a run of one command's events tagged 0 up CMD*n. */
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

/** A result written `$n` binds on first sight and must match after. */
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

/** A number a trace writes: a literal, `$n`, or `slot $n`, the slot a handle
 * names. */
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

/** A relation a trace says holds between what was seen: `A == B` or `A != B`. */
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

/** Any negative is none, as the contract says (ARCHITECTURE.md, the turn): each
 * language's `NO_WAKE` is its own pick of one. */
function wake_name(wake: number): string {
  return wake === 0 ? "next" : wake > 0 ? "later" : "none";
}

function run(
  g: Golden,
  guest: string,
  opener: Opener,
  trace: string,
  seed: number,
): void {
  const name = `${guest}, layout ${seed}: ${trace}`;
  clock_ms = 0;
  const host = new Host(opener.open(seed));
  const turns: Turn[] = [];
  const vars = new Map<string, number>();
  for (let i = 0; g.has(`${trace}.step.${i}`); i++) {
    const step = g.s(`${trace}.step.${i}`).split(" ");
    switch (step[0]) {
      case "submit": {
        host.submit(
          Number(step[1]),
          cmd_number(step[2]),
          step[3] === "ack",
          step.slice(4).map((t) => val(t, vars)),
        );
        break;
      }
      case "ring": {
        // A turn is held to the trace as soon as it is seen, so the results
        // it binds can be named by the steps after it.
        const turn = host.ring(Number(step[1]));
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
          turns.push(host.ring(0));
          if (!host.pending) break;
          assert.ok(
            turns.length < MOST_SETTLE_TURNS,
            `${name}: the guest stopped applying what waits`,
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
        host.word(k),
        SIZES[k],
        `${name}: the header's ${k}`,
      );
    }
    assert.deepStrictEqual(
      [
        host.ring_word("SQ_AT", ring.CAP),
        host.ring_word("CQ_AT", ring.CAP),
        host.ring_word("SQ_AT", ring.SLOT_BYTES),
        host.ring_word("CQ_AT", ring.SLOT_BYTES),
      ],
      [SQ_CAP, CQ_CAP, SLOT_BYTES, SLOT_BYTES],
      `${name}: the rings' own headers`,
    );
  }
  if (g.has(at("head.places"))) {
    for (const k of g.s(at("head.places")).split(" ")) {
      assert.notDeepStrictEqual(host.word(k), 0, `${name}: the header's ${k}`);
    }
  }
}

for (const trace of ["head", "door", "handle", "clock", "file", "call"]) {
  for (const [guest, open] of Object.entries(GUESTS)) {
    test(`trace ${trace} - the ${guest} guest`, async () => {
      const g = await Golden.load(`conformance/${trace}.tsv`);
      const opener = open();
      try {
        for (let seed = 0; seed < LAYOUTS; seed++) {
          for (const scenario of g.s("scenarios").split(" ")) {
            run(g, guest, opener, scenario, seed);
          }
        }
      } finally {
        opener.close();
      }
    });
  }
}

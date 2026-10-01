// @xpute/conformance/trace.test.ts

/**
 * The traces (spec/xpute/conformance/) run against every guest, by the one
 * host there is: this runtime's ring, directory and stream reader. A guest is
 * Rust or C++, and built as a native library the host opens or as a
 * WebAssembly module it instantiates; each exports the same two calls
 * (cpp/xpute/conformance/guest.hpp), so none is where another is held.
 *
 * The host knows only what the contract lets it: the directory's offset,
 * which the guest's boot answers, and then what the directory names. It sends
 * what a step says, rings, runs the stream and reads the completion ring dry
 * after every turn, as rule 5 asks of a host, and holds what it saw to what
 * the trace says it must see.
 *
 * Two rules are not in any trace because the host here is what keeps them:
 * the memory never moves (rule 1), and the host never runs while a turn does
 * (rule 2's exclusion). A trace holds what a guest could get wrong.
 *
 * The guests are built before this runs: the Rust ones by Cargo, the C++
 * ones by script/cpp.ts.
 */

import { assert, assertEquals, assertNotEquals } from "@std/assert";
import { fromFileUrl, join, resolve } from "@std/path";

import { Golden } from "@xpute/core/golden.ts";
import { encoder, TreeView } from "@xpute/core/wire/xtp/mod.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { handle_slot } from "@xpute/runtime/abi/handle.ts";
import { Directory } from "@xpute/runtime/ipc/directory.ts";
import { key as directory_key } from "@xpute/runtime/ipc/directory.spec.ts";
import { FrameFlag } from "@xpute/runtime/ipc/frame.ts";
import { Ring } from "@xpute/runtime/ipc/ring.ts";
import { each_record } from "@xpute/runtime/ipc/stream.ts";
import { cmd, key, MEMORY_BYTES } from "./guest.spec.ts";

const ROOT = fromFileUrl(new URL("../../../../", import.meta.url));
const CARGO_TARGET = resolve(Deno.env.get("CARGO_TARGET_DIR") ?? join(ROOT, "crates/target"));

/** A shared library under `dir`, its file named as the platform names one. */
function library(dir: string, name: string): string {
  return join(dir, Deno.build.os === "darwin" ? `lib${name}.dylib` : Deno.build.os === "windows" ? `${name}.dll` : `lib${name}.so`);
}

const SYMBOLS = {
  xpute_conformance_boot: { parameters: ["buffer", "function"], result: "u32" },
  xpute_conformance_interrupt: { parameters: ["buffer", "f64"], result: "f64" },
} as const;

/** The memory a host reserves a WebAssembly guest: the guest's range and
 * room past it for the module's own stack and data, which are the build's
 * and no number the host is told. A module that needs more says so by
 * failing to instantiate. */
const MODULE_BYTES = 4 << 20;

/** What a WebAssembly guest may import: its memory and the host's clock,
 * the contract's one call out, and nothing that would be a second way to the
 * host beside it. */
const MODULE_IMPORTS = ["env.memory", "env.now"];

/** A guest as the host holds one, however it was built: the memory, where
 * its boot said the directory is, and its turn. */
interface Guest {
  memory: ArrayBuffer;
  directory_at: number;
  interrupt(quota_ms: number): number;
}

/** A way to boot a fresh guest, and to let go of what it was opened from. */
interface Opener {
  open(): Guest;
  close(): void;
}

/** A guest built as a native library: booted into a region the host holds,
 * with the host's clock handed over. */
function native(path: string): Opener {
  const lib = Deno.dlopen(path, SYMBOLS);
  const now = new Deno.UnsafeCallback({ parameters: [], result: "f64" } as const, () => clock_ms);
  return {
    open() {
      // Words of eight bytes, so the region is 8-aligned, as a guest asks.
      const memory = new Float64Array(MEMORY_BYTES / 8).buffer;
      const bytes = new Uint8Array(memory);
      const directory_at = lib.symbols.xpute_conformance_boot(bytes, now.pointer);
      return { memory, directory_at, interrupt: (quota) => lib.symbols.xpute_conformance_interrupt(bytes, quota) };
    },
    close() {
      now.close();
      lib.close();
    },
  };
}

/** A guest built as a WebAssembly module: instantiated over a memory the
 * host reserves, which never grows, with the clock as its import. */
function module(path: string): Opener {
  const compiled = new WebAssembly.Module(Deno.readFileSync(path));
  const imports = WebAssembly.Module.imports(compiled).map((i) => `${i.module}.${i.name}`).sort();
  assertEquals(imports, MODULE_IMPORTS, `${path}: what the module reaches its host through`);
  return {
    open() {
      const pages = MODULE_BYTES >> 16;
      const memory = new WebAssembly.Memory({ initial: pages, maximum: pages });
      const instance = new WebAssembly.Instance(compiled, { env: { memory, now: () => clock_ms } });
      const exports = instance.exports as { xpute_conformance_boot(): number; xpute_conformance_interrupt(quota_ms: number): number };
      const directory_at = exports.xpute_conformance_boot() >>> 0;
      return { memory: memory.buffer as ArrayBuffer, directory_at, interrupt: (quota) => exports.xpute_conformance_interrupt(quota) };
    },
    close() {},
  };
}

/** Every guest, by how it is built: Cargo's builds and script/cpp.ts's. */
const GUESTS: Record<string, () => Opener> = {
  "rust native": () => native(library(join(CARGO_TARGET, "debug"), "xpute_conformance")),
  "cpp native": () => native(library(join(ROOT, "target/cpp"), "xpute_conformance")),
  "rust wasm": () => module(join(CARGO_TARGET, "wasm32-unknown-unknown/debug/xpute_conformance.wasm")),
  "cpp wasm": () => module(join(ROOT, "target/cpp/xpute_conformance.wasm")),
};

/** Turns a `settle` rings at most before the trace is taken to have stalled. */
const MOST_SETTLE_TURNS = 64;

/** The host's clock, which a `clock` step sets: a trace's times are its
 * inputs, and only what a guest makes of them is held to anything. */
let clock_ms = 0;

interface Completion {
  tag: number;
  cmd: number;
  reply: boolean;
  result: number;
}

/** What one turn left, each place on its own. */
interface Turn {
  completions: Completion[];
  stream: number[];
  wake: number;
}

class Host {
  readonly directory: Directory;
  /** The directory's word kept at 0. */
  readonly reserved: number;
  readonly sq: Ring;
  private readonly cq: Ring;
  private readonly stream_at: number;

  /** A fresh guest, found through the directory its boot pointed at. */
  constructor(private readonly guest: Guest) {
    const mem = guest.memory;
    this.directory = new Directory(mem, guest.directory_at);
    this.reserved = new Uint32Array(mem, guest.directory_at, 4)[3];
    this.sq = new Ring(mem, this.directory.get(directory_key.SUBMISSION));
    this.cq = new Ring(mem, this.directory.get(directory_key.COMPLETION));
    this.stream_at = this.directory.get(key.STREAM);
  }

  submit(tag: number, command: number, ack: boolean, arg: number | undefined): void {
    const packet = arg === undefined ? null : encoder.encode(new TreeView().branch((b) => b.u32(arg)));
    assertEquals(this.sq.push(tag, command, ack ? FrameFlag.ACKREQ : 0, packet), Errno.OK);
  }

  /** A turn, and everything it left taken before anything else. */
  ring(quota_ms: number): Turn {
    const wake = this.guest.interrupt(quota_ms);
    const stream: number[] = [];
    each_record(this.guest.memory, this.stream_at, 1, (op) => stream.push(op));
    const completions: Completion[] = [];
    for (let at = this.cq.peek(); at >= 0; at = this.cq.peek()) {
      completions.push({ tag: this.cq.tag(at), cmd: this.cq.cmd(at), reply: (this.cq.flags(at) & FrameFlag.RES) !== 0, result: this.cq.result(at) });
      this.cq.advance();
    }
    return { completions, stream, wake };
  }

  /** Whether the directory names `k`. */
  names(k: number): boolean {
    try {
      this.directory.get(k);
      return true;
    } catch {
      return false;
    }
  }
}

/** A directory key a trace names. */
function key_number(name: string): number {
  const keys: Record<string, number> = { SUBMISSION: directory_key.SUBMISSION, COMPLETION: directory_key.COMPLETION, STREAM: key.STREAM };
  if (!(name in keys)) throw new Error(`no key ${name}`);
  return keys[name];
}

const CMD_NAMES = Object.entries(cmd).filter(([, v]) => typeof v === "number") as [string, number][];

function cmd_number(name: string): number {
  const found = CMD_NAMES.find(([n]) => n === name);
  if (found) return found[1];
  const n = parseInt(name.replace(/^0x/, ""), 16);
  if (Number.isNaN(n)) throw new Error(`no command ${name}`);
  return n;
}

function cmd_name(n: number): string {
  return CMD_NAMES.find(([, v]) => v === n)?.[0] ?? `0x${n.toString(16).padStart(4, "0")}`;
}

/** A completion ring's sequence as a trace writes it: a reply TAG:CMD=RESULT,
 * a signal TAG:CMD, and a run of one command's signals tagged 0 up CMD*n. */
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

/** Holds what was seen to what a trace says, token by token. A result written
 * `$n` names a value the trace does not fix — a handle — and binds it the
 * first time, so a later step or relation can use it; after that it must be
 * the same. */
function expect(seen: string, said: string, vars: Map<string, number>, what: string): void {
  const seen_tokens = seen.split(" ");
  const said_tokens = said.split(" ");
  assertEquals(seen_tokens.length, said_tokens.length, `${what}: saw ${seen}, the trace says ${said}`);
  seen_tokens.forEach((seen_token, i) => {
    const said_token = said_tokens[i];
    const [seen_head, value] = seen_token.split("=");
    const [said_head, v] = said_token.split("=");
    if (v !== undefined && v.startsWith("$") && value !== undefined) {
      assertEquals(seen_head, said_head, `${what}: saw ${seen}, the trace says ${said}`);
      if (!vars.has(v)) vars.set(v, Number(value));
      assertEquals(vars.get(v), Number(value), `${what}: ${v} again, and not what it was`);
    } else {
      assertEquals(seen_token, said_token, `${what}: saw ${seen}, the trace says ${said}`);
    }
  });
}

/** A number a trace writes: a literal, `$n`, or `slot $n`, the slot a handle
 * names. */
function value(term: string, vars: Map<string, number>): number {
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
    assertEquals(value(a, vars), value(b, vars), `${what}: ${said}`);
  } else if (said.includes(" != ")) {
    const [a, b] = said.split(" != ");
    assertNotEquals(value(a, vars), value(b, vars), `${what}: ${said}`);
  } else {
    throw new Error(`${what}: no relation in ${said}`);
  }
}

function wake_name(wake: number): string {
  return wake === 0 ? "next" : wake > 0 ? "later" : "none";
}

/** Runs one scenario's steps on a fresh guest and holds every turn, and the
 * whole, to what the trace says of them. */
function run(g: Golden, guest: string, opener: Opener, trace: string): void {
  const name = `${guest}: ${trace}`;
  clock_ms = 0;
  const host = new Host(opener.open());
  const turns: Turn[] = [];
  const vars = new Map<string, number>();
  for (let i = 0; g.has(`${trace}.step.${i}`); i++) {
    const step = g.s(`${trace}.step.${i}`).split(" ");
    switch (step[0]) {
      case "submit":
        host.submit(Number(step[1]), cmd_number(step[2]), step[3] === "ack", step[4] === undefined ? undefined : value(step[4], vars));
        break;
      case "ring": {
        // A turn is held to the trace as soon as it is seen, so the results
        // it binds can be named by the steps after it.
        const turn = host.ring(Number(step[1]));
        const at = `${trace}.turn.${turns.length}.completions`;
        if (g.has(at)) expect(render_completions(turn.completions), g.s(at), vars, `${name} turn ${turns.length}`);
        turns.push(turn);
        break;
      }
      case "clock":
        clock_ms = Number(step[1]);
        break;
      case "settle":
        for (;;) {
          turns.push(host.ring(0));
          if (host.sq.len === 0) break;
          assert(turns.length < MOST_SETTLE_TURNS, `${name}: the guest stopped applying what waits`);
        }
        break;
      default:
        throw new Error(`${name}: no step ${step.join(" ")}`);
    }
  }
  turns.forEach((turn, t) => {
    const at = (what: string) => `${trace}.turn.${t}.${what}`;
    if (g.has(at("stream"))) assertEquals(render_stream(turn.stream), g.s(at("stream")), `${name} turn ${t}: the stream`);
    if (g.has(at("wake"))) assertEquals(wake_name(turn.wake), g.s(at("wake")), `${name} turn ${t}: what it asked for next`);
  });
  const at = (what: string) => `${trace}.${what}`;
  if (g.has(at("completions"))) assertEquals(render_completions(turns.flatMap((t) => t.completions)), g.s(at("completions")), `${name}: the completion ring`);
  if (g.has(at("stream"))) assertEquals(render_stream(turns.flatMap((t) => t.stream)), g.s(at("stream")), `${name}: the stream`);
  if (g.has(at("turns.least"))) assert(turns.length >= g.u32(at("turns.least")), `${name}: ${turns.length} turns`);
  if (g.has(at("wake.before_last"))) {
    turns.slice(0, -1).forEach((turn, t) => assertEquals(wake_name(turn.wake), g.s(at("wake.before_last")), `${name} turn ${t}: what it asked for next`));
  }
  if (g.has(at("wake.last"))) assertEquals(wake_name(turns[turns.length - 1].wake), g.s(at("wake.last")), `${name}: what the last turn asked for`);
  for (let r = 0; g.has(at(`relation.${r}`)); r++) relation(g.s(at(`relation.${r}`)), vars, name);
  if (g.has(at("directory.keys"))) {
    for (const k of g.s(at("directory.keys")).split(" ")) assert(host.names(key_number(k)), `${name}: the directory names no ${k}`);
  }
  if (g.has(at("directory.reserved"))) assertEquals(host.reserved, g.u32(at("directory.reserved")), `${name}: the directory's reserved word`);
}

for (const trace of ["directory", "door", "handle", "clock"]) {
  for (const [guest, open] of Object.entries(GUESTS)) {
    Deno.test(`trace ${trace} - the ${guest} guest`, async () => {
      const g = await Golden.load(`conformance/${trace}.tsv`);
      const opener = open();
      try {
        for (const scenario of g.s("scenarios").split(" ")) run(g, guest, opener, scenario);
      } finally {
        opener.close();
      }
    });
  }
}

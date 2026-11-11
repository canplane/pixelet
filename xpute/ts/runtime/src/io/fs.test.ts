// @xpute/runtime/io/fs.test.ts

/** Runs spec/conformance/fs/file.tsv against the file device. */

import { test } from "vitest";
import assert from "node:assert/strict";

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { Golden } from "../../../dev/golden.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { Branch, PacketWriter, TreeReader } from "@xpute/kit/wire/xtp/mod.ts";
import { Command, fs_imports } from "@xpute/runtime/abi/fs.spec.ts";
import { type Binding, FileError, FsServer } from "./fs.ts";
import { flag } from "./fs.spec.ts";
import type { Path } from "./path.ts";

const SCRATCH = 1 << 15;

const hex = (s: string): U8Array =>
  new Uint8Array((s.match(/../g) ?? []).map((b) => parseInt(b, 16)));
const text = new TextEncoder();

/** A `Turn` holds a turn's file calls, as the core sends them for the file
 * device, each with the memory it names. */
class Turn {
  readonly mem = new ArrayBuffer(1 << 16);
  private _frames: [number, number, U8Array, U8Array | null][] = [];

  post(
    tag: number,
    cmd: number,
    cnt: number,
    fill: (w: PacketWriter) => void,
    named: U8Array | null = null,
  ): void {
    const buf = new Uint8Array(1024);
    const w = new PacketWriter().begin(
      buf,
      new DataView(buf.buffer),
      0,
      1024,
      cnt,
    );
    fill(w);
    this._frames.push([tag, cmd, buf.slice(0, w.finish()), named]);
  }

  /** Hands the turn's frames to `server` as the runtime routes them, and
   * answers a refusal as the runtime does (io/connection.ts `_read`): a
   * failure is replied to, through `refused`. */
  take(
    server: FsServer,
    refused: (tag: number, cmd: number, res: number) => void,
  ): void {
    const listener = fs_imports(server).on;
    for (const [tag, cmd, pkt, named] of this._frames) {
      const f = pkt.byteLength > 0 ? new TreeReader(pkt).read_branch() : null;
      const res = listener(tag, cmd, f, pkt, named);
      if (res !== undefined && res < 0) refused(tag, cmd, res);
    }
    this._frames = [];
  }
}

/** A path as the trace writes it, a JSON array of its tokens. */
const path_of = (s: string): Path => JSON.parse(s);
/** The key a path's file is kept under, the same however the trace spaced
 * it. */
const key_of = (p: Path): string => JSON.stringify(p);

const flag_of = (s: string): number =>
  (flag as Record<string, number>)[s] ?? Number(s);

const errno = (name: string): number =>
  (Errno as unknown as Record<string, number>)[name];

async function run(g: Golden, scenario: string): Promise<void> {
  const turn = new Turn();
  const files = new Map<string, string>();
  const waiting = new Map<string, (bytes: U8Array) => void>();
  const failing = new Map<string, (err: FileError) => void>();
  const answers: [number, number, number][] = [];
  const commits = new Map<string, string>();
  const refusals = new Map<string, number>();
  const binding: Binding = {
    open: (path) => {
      const key = key_of(path);
      const b = files.get(key);
      if (b === undefined || b === "ENOENT") throw new FileError(Errno.ENOENT);
      if (b === "later") {
        return new Promise(
          (res, rej) => (waiting.set(key, res), failing.set(key, rej)),
        );
      }
      return hex(b);
    },
    commit: (path, bytes) => {
      const refused = refusals.get(key_of(path));
      if (refused !== undefined) throw new FileError(refused);
      commits.set(key_of(path), new TextDecoder().decode(bytes));
    },
  };
  const server = new FsServer(
    binding,
    turn.mem,
    (tag, cmd, res) => void answers.push([cmd, tag, res]),
  );
  const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

  const steps: string[] = [];
  g.each(`${scenario}.step`, (k) => steps.push(g.s(k)));
  for (const step of steps) {
    const [what, ...a] = step.split(" ");
    const at = `${scenario}: ${step}`;
    switch (what) {
      case "file":
        files.set(key_of(path_of(a[0])), a[1]);
        break;
      case "refuse":
        refusals.set(key_of(path_of(a[0])), errno(a[1]));
        break;
      case "open":
        turn.post(Number(a[0]), Command.FS_OPEN, 2, (w) =>
          w.arg(new Branch(path_of(a[1]))).u32(flag_of(a[2])),
        );
        break;
      case "read": {
        const at = a[2] === undefined ? SCRATCH : Number(a[2]);
        turn.post(Number(a[0]), Command.FS_READ, 2, (w) =>
          w.u32(at).u32(Number(a[1])),
        );
        break;
      }
      case "write": {
        const bytes = text.encode(a[1]);
        turn.post(
          Number(a[0]),
          Command.FS_WRITE,
          2,
          (w) => w.u32(SCRATCH).u32(bytes.length),
          bytes,
        );
        break;
      }
      case "close":
        turn.post(Number(a[0]), Command.FS_CLOSE, 0, () => {});
        break;
      case "turn":
        turn.take(
          server,
          (tag, cmd, res) => void answers.push([cmd, tag, res]),
        );
        await settle();
        break;
      case "deliver": {
        const key = key_of(path_of(a[0]));
        if (a[1] === "ENOENT") failing.get(key)?.(new FileError(Errno.ENOENT));
        else waiting.get(key)?.(hex(a[1]));
        waiting.delete(key);
        failing.delete(key);
        await settle();
        break;
      }
      case "expect":
        switch (a[0]) {
          case "opened":
          case "closed": {
            const want: [number, number, number] = [
              a[0] === "opened" ? Command.FS_OPEN : Command.FS_CLOSE,
              Number(a[1]),
              /^\d+$/.test(a[2]) ? Number(a[2]) : errno(a[2]),
            ];
            assert.deepStrictEqual(answers.shift(), want, at);
            break;
          }
          case "quiet":
            assert.deepStrictEqual(answers, [], at);
            break;
          case "read":
            assert.deepStrictEqual(
              Array.from(new Uint8Array(turn.mem, SCRATCH, a[1].length / 2)),
              Array.from(hex(a[1])),
              at,
            );
            break;
          case "commit":
            assert.deepStrictEqual(
              commits.get(key_of(path_of(a[1]))),
              a[2],
              at,
            );
            break;
          case "unwritten":
            assert.ok(!commits.has(key_of(path_of(a[1]))), at);
            break;
          default:
            throw new Error(`${at}: no such expectation`);
        }
        break;
      default:
        throw new Error(`${at}: no such step`);
    }
  }
}

test("the file service keeps the trace", async () => {
  const g = await Golden.load("conformance/fs/file.tsv");
  for (const scenario of g.s("scenarios").split(" ")) await run(g, scenario);
});

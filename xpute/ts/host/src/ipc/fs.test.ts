// @xpute/host/ipc/fs.test.ts

/** Runs spec/conformance/fs/file.tsv against the file device. */

import { test } from "vitest";
import assert from "node:assert/strict";

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { Golden } from "@xpute/kit/golden.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { PacketWriter, TreeReader } from "@xpute/kit/wire/xtp/mod.ts";
import { Command, fs_imports } from "@xpute/host/abi/fs.spec.ts";
import { FileError, type Fs, FsServer } from "./fs.ts";
import { flag } from "./fs.spec.ts";

const SCRATCH = 1 << 15;

const hex = (s: string): U8Array =>
  new Uint8Array((s.match(/../g) ?? []).map((b) => parseInt(b, 16)));
const text = new TextEncoder();

/** A turn's file calls, as the guest posts them for the file device. */
class Turn {
  readonly mem = new ArrayBuffer(1 << 16);
  writes = 0;
  private frames: [number, U8Array][] = [];

  post(cmd: number, cnt: number, fill: (w: PacketWriter) => void): void {
    const buf = new Uint8Array(1024);
    const w = new PacketWriter().begin(
      buf,
      new DataView(buf.buffer),
      0,
      1024,
      cnt,
    );
    fill(w);
    this.frames.push([cmd, buf.slice(0, w.finish())]);
  }

  /** The frames handed to the device, as its host hands them. */
  take(server: FsServer): void {
    const heard = fs_imports(server);
    for (const [cmd, pkt] of this.frames) {
      heard(0, cmd, new TreeReader(pkt).read_branch());
    }
    this.frames = [];
  }
}

const ERRNO: Record<string, number> = {
  ENOENT: Errno.ENOENT,
  EACCES: Errno.EACCES,
};
const errno = (name: string): number => ERRNO[name];

async function run(g: Golden, scenario: string): Promise<void> {
  const turn = new Turn();
  const files = new Map<string, string>();
  const waiting = new Map<string, (bytes: U8Array) => void>();
  const failing = new Map<string, (err: FileError) => void>();
  const answers: [number, number][] = [];
  const commits = new Map<string, string>();
  const fs: Fs = {
    open: (root, path) => {
      const key = `${root} ${path}`;
      const b = files.get(key);
      if (b === undefined || b === "ENOENT") throw new FileError(Errno.ENOENT);
      if (b === "later") {
        return new Promise(
          (res, rej) => (waiting.set(key, res), failing.set(key, rej)),
        );
      }
      return hex(b);
    },
    commit: (root, path, bytes) =>
      void commits.set(`${root} ${path}`, new TextDecoder().decode(bytes)),
  };
  const server = new FsServer(
    fs,
    turn.mem,
    (fd, res) => void answers.push([fd, res]),
  );
  const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

  const steps: string[] = [];
  g.each(`${scenario}.step`, (k) => steps.push(g.s(k)));
  for (const step of steps) {
    const [what, ...a] = step.split(" ");
    const at = `${scenario}: ${step}`;
    const path = (p: string) => (p === '""' ? "" : p);
    switch (what) {
      case "file":
        files.set(`${a[0]} ${a[1]}`, a[2]);
        break;
      case "openat":
        turn.post(Command.FS_OPENAT, 4, (w) =>
          w
            .u32(Number(a[0]))
            .u32(Number(a[1]))
            .u32(a[2] === "wr" ? flag.O_WRONLY : flag.O_RDONLY)
            .str(path(a[3])),
        );
        break;
      case "read":
        turn.post(Command.FS_READ, 3, (w) =>
          w.u32(Number(a[0])).u32(SCRATCH).u32(Number(a[1])),
        );
        break;
      case "write": {
        const bytes = text.encode(a[1]);
        // Each write its own bytes: the host copies them only once the turn is taken.
        const at_ = SCRATCH + 4096 + 64 * turn.writes++;
        new Uint8Array(turn.mem, at_, bytes.length).set(bytes);
        turn.post(Command.FS_WRITE, 3, (w) =>
          w.u32(Number(a[0])).u32(at_).u32(bytes.length),
        );
        break;
      }
      case "close":
        turn.post(Command.FS_CLOSE, 1, (w) => w.u32(Number(a[0])));
        break;
      case "turn":
        turn.take(server);
        await settle();
        break;
      case "deliver": {
        const key = `${a[0]} ${a[1]}`;
        if (a[2] === "ENOENT") failing.get(key)?.(new FileError(Errno.ENOENT));
        else waiting.get(key)?.(hex(a[2]));
        waiting.delete(key);
        failing.delete(key);
        await settle();
        break;
      }
      case "expect":
        switch (a[0]) {
          case "opened": {
            const want: [number, number] = [
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
            assert.deepStrictEqual(commits.get(`${a[1]} ${a[2]}`), a[3], at);
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

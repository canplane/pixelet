// @xpute/runtime/ipc/sys.test.ts

/** Runs spec/xpute/conformance/sys/file.tsv against the host's end of the system calls. */

import { assertEquals } from "@std/assert";

import type { U8Array } from "@xpute/core/abi/array.ts";
import { Golden } from "@xpute/core/golden.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { END } from "./stream.ts";
import { FileError, Sys, type SysHost } from "./sys.ts";
import { flag, HEAD, op } from "./sys.spec.ts";

const SCRATCH = 1 << 15;

const hex = (s: string): U8Array => new Uint8Array((s.match(/../g) ?? []).map((b) => parseInt(b, 16)));
const text = new TextEncoder();

class Turn {
  readonly memory = new ArrayBuffer(1 << 16);
  writes = 0;
  private words: number[] = [];

  record(code: number, args: number[], bytes?: U8Array): void {
    const body = [...args];
    if (bytes) {
      body.push(bytes.length);
      const padded = new Uint8Array(Math.ceil(bytes.length / 4) * 4);
      padded.set(bytes);
      body.push(...new Uint32Array(padded.buffer));
    }
    this.words.push(code, body.length, ...body);
  }

  publish(): void {
    const w = new Uint32Array(this.memory, 0, HEAD + this.words.length);
    w.fill(0);
    w[END] = this.words.length;
    w.set(this.words, HEAD);
    this.words = [];
  }
}

const ERRNO: Record<string, number> = { ENOENT: Errno.ENOENT, EACCES: Errno.EACCES };
const errno = (name: string): number => ERRNO[name];

async function run(g: Golden, scenario: string): Promise<void> {
  const turn = new Turn();
  const kept = new Map<string, string>();
  const waiting = new Map<string, (bytes: U8Array) => void>();
  const failing = new Map<string, (err: FileError) => void>();
  const answers: [number, number][] = [];
  const commits = new Map<string, string>();
  const host: SysHost = {
    open: (root, path) => {
      const key = `${root} ${path}`;
      const b = kept.get(key);
      if (b === undefined || b === "ENOENT") throw new FileError(Errno.ENOENT);
      if (b === "later") return new Promise((res, rej) => (waiting.set(key, res), failing.set(key, rej)));
      return hex(b);
    },
    commit: (root, path, bytes) => void commits.set(`${root} ${path}`, new TextDecoder().decode(bytes)),
    opened: (fd, res) => void answers.push([fd, res]),
    abort: () => {},
    program: () => {},
  };
  const sys = new Sys(host);
  const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

  const steps: string[] = [];
  g.each(`${scenario}.step`, (k) => steps.push(g.s(k)));
  for (const step of steps) {
    const [what, ...a] = step.split(" ");
    const at = `${scenario}: ${step}`;
    const path = (p: string) => (p === '""' ? "" : p);
    switch (what) {
      case "file":
        kept.set(`${a[0]} ${a[1]}`, a[2]);
        break;
      case "openat":
        turn.record(op.OPENAT, [Number(a[0]), Number(a[1]), a[2] === "wr" ? flag.O_WRONLY : flag.O_RDONLY], text.encode(path(a[3])));
        break;
      case "read":
        turn.record(op.READ, [Number(a[0]), SCRATCH, Number(a[1])]);
        break;
      case "write": {
        const bytes = text.encode(a[1]);
        // Each write its own bytes: the host copies them only once the turn is run.
        const at_ = SCRATCH + 4096 + 64 * turn.writes++;
        new Uint8Array(turn.memory, at_, bytes.length).set(bytes);
        turn.record(op.WRITE, [Number(a[0]), at_, bytes.length]);
        break;
      }
      case "close":
        turn.record(op.CLOSE, [Number(a[0])]);
        break;
      case "turn":
        turn.publish();
        sys.run(turn.memory, 0);
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
            const want: [number, number] = [Number(a[1]), /^\d+$/.test(a[2]) ? Number(a[2]) : errno(a[2])];
            assertEquals(answers.shift(), want, at);
            break;
          }
          case "quiet":
            assertEquals(answers, [], at);
            break;
          case "read":
            assertEquals(Array.from(new Uint8Array(turn.memory, SCRATCH, a[1].length / 2)), Array.from(hex(a[1])), at);
            break;
          case "commit":
            assertEquals(commits.get(`${a[1]} ${a[2]}`), a[3], at);
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

Deno.test("the file service keeps the trace", async () => {
  const g = await Golden.load("conformance/sys/file.tsv");
  for (const scenario of g.s("scenarios").split(" ")) await run(g, scenario);
});

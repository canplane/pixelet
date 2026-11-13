// @pxl/client/service/sys/fs/file.ts

/**
 * The core's files (xpute wit/fs/file.wit), served from the binding behind
 * their paths (binding.ts), one opener mounted at each node atop file.json's
 * `path` tree. An open answers at once and starts the file's read; what the
 * read comes to, its bytes or its error, waits in the file for its first
 * `read`. What is written is committed at the file's close, which answers EIO
 * where the commit failed; and the close cancels what is in flight on the file:
 * a method waiting on it answers ECANCELED. A read or a wait for a change its
 * caller cancels answers ECANCELED too (frame.json `CANCEL`).
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { FaultError } from "@xpute/kit/status/error.ts";
import type { Link } from "@xpute/link";
import * as file from "@xpute/link/abi/fs/file.ts";

import { path } from "@/abi/file.spec.ts";
import { binding, type Path } from "@/service/sys/fs/binding.ts";

/** What waits for each path to change, by the path as text. */
const waiting = new Map<string, (() => void)[]>();

/** Tells the files open at `p` that it changed under them. */
export function changed(p: Path): void {
  const at = JSON.stringify(p);
  const done = waiting.get(at) ?? [];
  waiting.delete(at);
  for (const d of done) d();
}

/** What rejects ECANCELED once `signal` aborts, its caller having canceled
 * the method. */
function aborted(signal: AbortSignal): Promise<never> {
  return new Promise((_, fail) => {
    signal.addEventListener("abort", () => fail(new FaultError(Errno.ECANCELED)));
  });
}

/** The file at `p`, over the core's memory `link` lends. */
function open(link: Link, p: Path): file.File {
  const b = binding();
  const abort = new AbortController();
  const opened = Promise.resolve().then(() => b.open(p, abort.signal));
  // What the read comes to is the first method's to answer.
  opened.catch(() => {});
  /** The file's bytes, or ECANCELED once it is closed. */
  const bytes = (): Promise<U8Array> =>
    abort.signal.aborted
      ? Promise.reject(new FaultError(Errno.ECANCELED))
      : opened.catch((e: unknown) => {
          throw abort.signal.aborted ? new FaultError(Errno.ECANCELED) : e;
        });
  let read = 0;
  const written: U8Array[] = [];
  /** What waits for the file to change, which its close cancels. */
  let change: ((e?: unknown) => void) | null = null;
  return {
    async size() {
      return (await bytes()).length;
    },
    async read(buf, signal) {
      // A read its caller cancels writes nothing into `buf`.
      const all = await Promise.race([bytes(), aborted(signal)]);
      const n = Math.min(buf.len, all.length - read);
      link.bytes(buf.at, n).set(all.subarray(read, read + n));
      read += n;
      return n;
    },
    async write(buf) {
      written.push(link.bytes(buf.at, buf.len).slice());
    },
    changed(signal) {
      const at = JSON.stringify(p);
      return new Promise<void>((done, fail) => {
        const wake = () => done();
        const cancel = (e?: unknown) => {
          waiting.set(at, (waiting.get(at) ?? []).filter((w) => w !== wake));
          fail(e);
        };
        change = cancel;
        waiting.set(at, [...(waiting.get(at) ?? []), wake]);
        signal.addEventListener("abort", () => cancel(new FaultError(Errno.ECANCELED)));
      });
    },
    async close() {
      abort.abort();
      change?.(new FaultError(Errno.ECANCELED));
      if (written.length === 0) return;
      const all = new Uint8Array(written.reduce((n, w) => n + w.length, 0));
      written.reduce((at, w) => (all.set(w, at), at + w.length), 0);
      try {
        await b.commit(p, all);
      } catch {
        throw new FaultError(Errno.EIO);
      }
    },
  };
}

/** Mounts an opener at each node atop file.json's `path` tree. */
export function serve_files(link: Link): void {
  for (const node of Object.values(path)) {
    const id = (node as { ID?: number }).ID;
    if (id === undefined) continue;
    link.mount([{ tag: "int", val: id }], (uri) => file.file(link, open(link, uri.map((t) => t.val))));
  }
}

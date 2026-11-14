// @pxl/client/service/sys/fs/file.ts

/**
 * The core's files (xpute wit/fs/file.wit), served from the binding behind
 * their paths (binding.ts), one opener mounted at each node atop file.json5's
 * `path` tree. An open answers at once and starts the file's read; what the
 * read comes to, its bytes or its error, waits in the file, and each `size`
 * and `read` answers from it. What is written is committed at the file's
 * close; and the close cancels what is in flight on the file: a method waiting
 * on it answers ECANCELED. A size or a read its caller cancels answers
 * ECANCELED too (frame.json5 `CANCEL`). A set-aside goes to the backend the
 * path names (binding.ts). A backend fails with the file's error-code, and a
 * failure that carries none answers `io`, save a bug, which passes on as
 * itself (`failure`).
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { Bug } from "@xpute/kit/status/bug.ts";
import { ERRNO_ECANCELED } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { ComponentError, type Side } from "@xpute/link";
import * as file from "@xpute/link/abi/fs/file.ts";
import { ERROR_CODE_IO } from "@xpute/link/abi/fs/types.enum.ts";

import { path } from "@/abi/file.spec.ts";
import { binding, type Path } from "@/service/sys/fs/binding.ts";

/** What a backend's failure `e` answers: its own error-code, `io` where it
 * carries none, and a bug as itself. */
const failure = (e: unknown): unknown =>
  e instanceof ComponentError || e instanceof Bug
    ? e
    : new ComponentError(ERROR_CODE_IO);

/** What rejects ECANCELED once `signal` aborts: a method its caller canceled,
 * or a file closed. */
function aborted(signal: AbortSignal): Promise<never> {
  return new Promise((_, fail) => {
    signal.addEventListener("abort", () => fail(new XputeError(ERRNO_ECANCELED)));
  });
}

/** The file at `p`, over the core's memory `link` lends. */
function open(link: Pick<Side, "bytes">, p: Path): file.HostFile {
  const b = binding();
  const abort = new AbortController();
  const opened = Promise.resolve().then(() => b.open(p, abort.signal));
  // What the read comes to is the first method's to answer.
  opened.catch(() => {});
  /** ECANCELED once the file is closed. */
  const closed = aborted(abort.signal);
  closed.catch(() => {});
  /** The file's bytes, or ECANCELED once it is closed, whether or not its
   * backend heeds the close. */
  const bytes = (): Promise<U8Array> =>
    Promise.race([
      opened.catch((e: unknown) => {
        throw failure(e);
      }),
      closed,
    ]);
  let read = 0;
  const written: U8Array[] = [];
  return {
    async size(signal) {
      return (await Promise.race([bytes(), aborted(signal)])).length;
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
    async set_aside() {
      try {
        await b.set_aside(p, Date.now());
      } catch (e) {
        throw failure(e);
      }
    },
    async close() {
      abort.abort();
      if (written.length === 0) return;
      const all = new Uint8Array(written.reduce((n, w) => n + w.length, 0));
      written.reduce((at, w) => (all.set(w, at), at + w.length), 0);
      try {
        await b.commit(p, all);
      } catch (e) {
        throw failure(e);
      }
    },
    // A file dropped unclosed lets its bytes go and commits nothing.
    drop() {
      abort.abort();
    },
  };
}

/** Mounts an opener at each node atop file.json5's `path` tree. */
export function serve_files(
  link: Pick<Side, "bytes" | "mount" | "task_return" | "on_cancel">,
): void {
  for (const node of Object.values(path)) {
    const id = (node as { ID?: number }).ID;
    if (id === undefined) continue;
    link.mount(file.HANDLE, [{ tag: "int", val: id }], (uri) => file.file(link, open(link, uri.map((t) => t.val))));
  }
}

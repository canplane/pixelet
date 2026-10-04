// @pxl/web/kernel/host.ts

/**
 * The kernel's system calls as its host answers them (crates/pxl/web/kernel/
 * src/host.rs): xpute's, run by xpute (@xpute/runtime/ipc/sys.ts), with
 * storage bound here and the other roots bound by the boot (`bind_root`,
 * kernel.ts) — a bake to its origin, the assets to theirs — and a broken
 * invariant said on the console; and the kernel's own (abi/host.json), a text laid out
 * or drawn (crates/pxl/web/kernel/src/text.rs). All of them run once the turn
 * has returned and before the next can begin, so whatever one names in the
 * kernel's memory is written by the time the kernel reads it.
 */

import type { U32Array, U8Array } from "@xpute/core/abi/array.ts";
import type { u32 } from "@xpute/core/abi/word.ts";
import { td, te } from "@xpute/core/codec/encoding.ts";
import { Errno, strerror } from "@xpute/core/status/errno.spec.ts";
import { text_of } from "@xpute/runtime/ipc/stream.ts";
import { Command as XputeCommand, encode_sys_opened } from "@xpute/runtime/abi/cmd.spec.ts";
import { FileError, Sys } from "@xpute/runtime/ipc/sys.ts";

import { op, root } from "@/kernel/abi/host.spec.ts";
import { send_next } from "@/kernel/sched.ts";
import { storageGet, storageSet } from "@platform/storage.ts";
import { textRaster, textWidth } from "@platform/text.ts";

const word = new DataView(new ArrayBuffer(4));

function float(bits: u32): number {
  word.setUint32(0, bits, true);
  return word.getFloat32(0, true);
}

/** Where `text` breaks into lines no wider than `max_width` (0 for one
 * line), measured by `width`: each line's end, a UTF-16 index, the last the
 * text's end. Greedy by words; a line still too wide on its own — one long
 * word, or a message typed without spaces, as Korean often is — breaks
 * between characters. A break at a space leaves the space at the head of the
 * next line, for the kernel to trim. */
function line_ends(text: string, max_width: number, width: (s: string) => number): number[] {
  const words = [...text.matchAll(/\S+/g)].map((m) => [m.index, m.index + m[0].length]);
  if (max_width <= 0 || words.length === 0) return [text.length];
  const spans: [number, number][] = [];
  let [start, end] = words[0];
  for (let i = 1; i < words.length; i++) {
    const [ws, we] = words[i];
    if (width(text.slice(start, we)) <= max_width) end = we;
    else {
      spans.push([start, end]);
      [start, end] = [ws, we];
    }
  }
  spans.push([start, end]);
  const ends: number[] = [];
  for (const [s, e] of spans) {
    if (width(text.slice(s, e)) > max_width) {
      let chunk = s;
      for (let i = s; i < e;) {
        const next = i + (text.codePointAt(i)! > 0xffff ? 2 : 1);
        if (i > chunk && width(text.slice(chunk, next)) > max_width) {
          ends.push(i);
          chunk = i;
        }
        i = next;
      }
    }
    ends.push(e);
  }
  ends[ends.length - 1] = text.length;
  return ends;
}

/** The kernel's own calls: a text laid out or drawn, an image decoded. */
function program(code: u32, w: U32Array, a: u32): void {
  switch (code) {
    case op.IMAGE_DECODE: {
      // Taken now: the kernel may close the file in this turn.
      const [fd, image] = [w[a], w[a + 1]];
      const bytes = FILES.bytes(fd);
      // Answered after the record run, never inside it, as an open is
      // (@xpute/runtime/ipc/sys.ts).
      void (async () => {
        await Promise.resolve();
        const decoded = bytes !== null && (await decodeImage(image, bytes));
        send_next(Command.IMAGE_DECODED, encode_image_decoded({ image, res: decoded ? 0 : Errno.EBADMSG }));
      })();
      break;
    }
    case op.TEXT_LAYOUT: {
      const [weight, px, max_width, at, lines] = [w[a], float(w[a + 1]), float(w[a + 2]), w[a + 3], w[a + 4]];
      const text = text_of(w, a + 5);
      const width = (s: string) => textWidth(weight, px, s);
      const ends = line_ends(text, max_width, width).slice(0, lines);
      const out = new DataView(w.buffer, at, 4 * (1 + 2 * lines));
      out.setUint32(0, ends.length, true);
      let start = 0;
      let start_byte = 0;
      ends.forEach((end, i) => {
        const line = text.slice(start, end);
        start_byte += te.encode(line).length;
        out.setUint32(4 * (1 + 2 * i), start_byte, true);
        out.setFloat32(4 * (2 + 2 * i), width(line.trim()), true);
        start = end;
      });
      break;
    }
    case op.TEXT_RASTER: {
      const [weight, px, rgba, at, width, height, x] = [w[a], float(w[a + 1]), w[a + 2], w[a + 3], w[a + 4], w[a + 5], float(w[a + 6])];
      textRaster(weight, px, rgba, text_of(w, a + 7), x, width, height, new Uint8Array(w.buffer, at, width * height * 4));
      break;
    }
    default:
      throw new Error(`[host] call ${code}`);
  }
}

/** The kernel's open files. */
const FILES = new Files();

/** How a root's file is read whole. */
type Open = (path: string, signal: AbortSignal) => U8Array | Promise<U8Array>;

const ROOTS = new Map<u32, Open>([[root.STORAGE, (path) => {
  const value = storageGet(path);
  if (value === null) throw new FileError(Errno.ENOENT);
  return te.encode(value);
}]]);

/** Binds `r` to where its files are read from. A root bound to nothing has
 * no files. */
export function bind_root(r: u32, open: Open): void {
  ROOTS.set(r, open);
}

/** A file below one of the kernel's roots, whole. */
function open(r: u32, path: string, signal: AbortSignal) {
  const bound = ROOTS.get(r);
  if (bound === undefined) throw new FileError(Errno.ENOENT);
  return bound(path, signal);
}

/** The kernel's system calls and the files it holds open. */
const SYS = new Sys({
  open,
  commit: (r, path, bytes) => {
    if (r !== root.STORAGE) throw new FileError(Errno.EROFS);
    storageSet(path, td.decode(bytes));
  },
  opened: (fd, res, detail) => send_next(XputeCommand.SYS_OPENED, encode_sys_opened({ fd, res, detail })),
  abort: ({ errno, file, line, column, a, b }) => console.error(`[pxl] ${strerror(errno)} at ${file.replace(/^\.+/, "")}:${line}:${column} (${a}, ${b})`),
  program,
});

/** Runs the calls the turn just returned left in the stream at `at`. */
export function run_host_stream(memory: ArrayBuffer, at: u32): void {
  SYS.run(memory, at);
}

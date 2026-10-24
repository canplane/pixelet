// @pxl/client/kernel/host.ts

/**
 * The kernel's system calls as its host answers them: xpute's, plus text
 * layout and raster. All run between turns, so what they write is in place
 * by the time the kernel reads it.
 */

import type { U32Array, U8Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { td, te } from "@xpute/kit/codec/encoding.ts";
import { Errno, strerror } from "@xpute/kit/status/errno.spec.ts";
import { text_of } from "@xpute/host/ipc/stream.ts";
import { Command as XputeCommand, encode_sys_opened } from "@xpute/host/abi/cmd.spec.ts";
import { FileError, Sys } from "@xpute/host/ipc/sys.ts";

import { OBJECTS_PATH, op, root, SETTINGS_PATH } from "@/kernel/abi/host.spec.ts";
import { GUEST_DOORBELL } from "@/kernel/mem.ts";
import { send_next } from "@/kernel/sched.ts";
import { storageGet, storageSet } from "@/bracket.tsx";
import { textRaster, textWidth } from "@/bracket.tsx";
import { BUILD_NUMBER } from "@/config/deploy.ts";

const word = new DataView(new ArrayBuffer(4));

function float(bits: u32): number {
  word.setUint32(0, bits, true);
  return word.getFloat32(0, true);
}

/** Greedy by words; a word too wide alone (Korean typed without spaces)
 * breaks between characters. A space stays at the next line's head for the
 * kernel to trim. */
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

function program(code: u32, w: U32Array, a: u32): void {
  switch (code) {
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

type Open = (path: string, signal: AbortSignal) => U8Array | Promise<U8Array>;

/** STORAGE's files, each kept under its path, so a path's version is its key's. */
const STORAGE_PATHS: readonly string[] = [SETTINGS_PATH, OBJECTS_PATH];

function storage_key(path: string): string {
  if (!STORAGE_PATHS.includes(path)) throw new FileError(Errno.ENOENT);
  return `pxl:${path.replaceAll("/", ":")}`;
}

/** A STORAGE file's text, or null where none is kept. */
export function read_storage(path: string): string | null {
  return storageGet(storage_key(path));
}

const ROOTS = new Map<u32, Open>([[root.STORAGE, (path) => {
  const value = read_storage(path);
  if (value === null) throw new FileError(Errno.ENOENT);
  return te.encode(value);
}]]);

export function bind_root(r: u32, open: Open): void {
  ROOTS.set(r, open);
}

function open(r: u32, path: string, signal: AbortSignal) {
  const bound = ROOTS.get(r);
  if (bound === undefined) throw new FileError(Errno.ENOENT);
  return bound(path, signal);
}

let stopCode: string | null = null;

/** The kernel's last report, short as a stop code: its errno's name and the
 * file and line, a published build's file a number its paths decode by. None
 * where it stopped without one. */
export function stop_code(): string | null {
  return stopCode;
}

const SYS = new Sys({
  open,
  commit: (r, path, bytes) => {
    if (r !== root.STORAGE) throw new FileError(Errno.EROFS);
    storageSet(storage_key(path), td.decode(bytes));
  },
  opened: (fd, res, detail) => send_next(XputeCommand.SYS_OPENED, encode_sys_opened(GUEST_DOORBELL, fd, res, detail)),
  abort: ({ errno, file, line, column, a, b }) => {
    console.error(`[kernel] stopped: ${strerror(errno)} at ${file.replace(/^\.+/, "")}:${line}:${column} (${a}, ${b})`);
    const where = `${file.split("/").pop()}:${line}`;
    stopCode = `${strerror(errno).split(":")[0]} · ${file.startsWith("#") ? `${where} · ${BUILD_NUMBER}` : where}`;
  },
  program,
});

export function run_host_stream(memory: ArrayBuffer, at: u32): void {
  SYS.run(memory, at);
}

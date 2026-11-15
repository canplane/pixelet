// @xpute/golden

/**
 * Reads golden records, files of lines `dotted.path<TAB>value` under a
 * part's spec/, as the Rust and C++ readers do. A record holds every language that reads it
 * to the same answer, so only a record that crosses languages belongs here.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { f64, i32, u32, u64 } from "@xpute/kit/abi/word.ts";
import { ERRNO_EINVAL, ERRNO_ENOENT } from "@xpute/kit/status/errno.spec.ts";
import { bug } from "@xpute/kit/status/bug.ts";

/** Where the tree's root is, as whoever runs the tests knows it and defines
 * it: vitest's config in Node (vitest.config.ts), or a browser runner for its
 * page. Every record is read under it. */
declare const GOLDEN_ROOT: string;

export class Golden {
  private readonly map: Map<string, string>;

  private constructor(src: string) {
    this.map = new Map(
      src
        .split("\n")
        .filter((l) => l.includes("\t"))
        .map((l) => {
          const at = l.indexOf("\t");
          return [l.slice(0, at), l.slice(at + 1)] as const;
        }),
    );
  }

  /** Loads the record at `path` under xpute's root, such as
   * `kit/spec/golden/wire/tlv.tsv`. */
  static async load(path: string): Promise<Golden> {
    const root = new URL(GOLDEN_ROOT, import.meta.url);
    return new Golden(await (await fetch(new URL(path, root))).text());
  }

  has(key: string): boolean {
    return this.map.has(key);
  }

  s(key: string): string {
    const v = this.map.get(key);
    if (v === undefined) bug(ERRNO_ENOENT);
    return v;
  }

  u32(key: string): u32 {
    return Number(this.s(key));
  }

  i32(key: string): i32 {
    return Number(this.s(key));
  }

  u64(key: string): u64 {
    return BigInt(this.s(key));
  }

  /**
   * A double recorded as the hex of its bits, so it reads back exactly, -0
   * and a NaN's payload included.
   */
  f64(key: string): f64 {
    const bits = BigInt(this.s(key));
    const view = new DataView(new ArrayBuffer(8));
    view.setBigUint64(0, bits);
    return view.getFloat64(0);
  }

  bool(key: string): boolean {
    const v = this.s(key);
    if (v === "true") return true;
    if (v === "false") return false;
    bug(ERRNO_EINVAL);
  }

  bytes(key: string): U8Array {
    const text = this.s(key);
    const out = new Uint8Array(text.length / 2);
    for (let i = 0; i < out.length; i++) {
      out[i] = parseInt(text.slice(i * 2, i * 2 + 2), 16);
    }
    return out;
  }

  /**
   * Calls `f` with `base.0`, `base.1` and so on, for as long as some record's
   * key starts with that prefix. A list with no item is a bug.
   */
  each(base: string, f: (key: string) => void): void {
    let k = 0;
    for (; ; k++) {
      const prefix = base === "" ? `${k}` : `${base}.${k}`;
      const dotted = `${prefix}.`;
      let found = false;
      for (const key of this.map.keys()) {
        if (key === prefix || key.startsWith(dotted)) {
          found = true;
          break;
        }
      }
      if (!found) break;
      f(prefix);
    }
    if (k === 0) bug(ERRNO_ENOENT);
  }

  len(base: string): u32 {
    let n = 0;
    this.each(base, () => n++);
    return n;
  }
}

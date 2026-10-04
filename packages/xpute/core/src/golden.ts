// @xpute/core/golden.ts

/**
 * Reads the golden records under spec/xpute/ (lines of `path<TAB>value`), as
 * xpute-core/golden.rs does. Only records that cross languages belong here.
 */

import type { U8Array } from "./abi/array.ts";
import type { f64, u32, u64 } from "./abi/word.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { InvariantError } from "@xpute/core/status/error.ts";
const SPEC = new URL("../../../../spec/xpute/", import.meta.url);

export class Golden {
  private readonly map: Map<string, string>;

  private constructor(src: string) {
    this.map = new Map(
      src.split("\n").filter((l) => l.includes("\t")).map((l) => {
        const at = l.indexOf("\t");
        return [l.slice(0, at), l.slice(at + 1)] as const;
      }),
    );
  }

  /** The record at `path` under spec/xpute/: `golden/wire/tlv.tsv`. */
  static async load(path: string): Promise<Golden> {
    return new Golden(await Deno.readTextFile(new URL(path, SPEC)));
  }

  has(key: string): boolean {
    return this.map.has(key);
  }

  s(key: string): string {
    const v = this.map.get(key);
    if (v === undefined) throw new InvariantError(Errno.ENOENT);
    return v;
  }

  u32(key: string): u32 {
    return Number(this.s(key));
  }

  u64(key: string): u64 {
    return BigInt(this.s(key));
  }

  /** A double written through its bits, so that -0 and a NaN's payload survive. */
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
    throw new InvariantError(Errno.EINVAL);
  }

  bytes(key: string): U8Array {
    const text = this.s(key);
    const out = new Uint8Array(text.length / 2);
    for (let i = 0; i < out.length; i++) out[i] = parseInt(text.slice(i * 2, i * 2 + 2), 16);
    return out;
  }

  /** Calls `f` with `base.0`, `base.1`, … while an entry has that prefix. */
  each(base: string, f: (key: string) => void): void {
    let k = 0;
    for (;; k++) {
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
    if (k === 0) throw new InvariantError(Errno.ENOENT);
  }

  len(base: string): u32 {
    let n = 0;
    this.each(base, () => n++);
    return n;
  }
}

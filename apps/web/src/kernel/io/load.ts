// @pxl/web/kernel/io/load.ts

import type { U8Array } from "@xpute/core/abi/array.ts";
import type { u32 } from "@xpute/core/abi/word.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { FileError } from "@xpute/runtime/ipc/sys.ts";
import { loadKey } from "@/catalog/asset/load.ts";
import { originGet } from "@platform/origin.ts";
import { assetUrl } from "@/config/route.ts";
import { begin_read } from "@/kernel/io/fetch.ts";
import { CORE_DOORBELL } from "@/kernel/mem.ts";
import { BODIES } from "@/kernel/module.ts";
import { holdImage } from "@platform/image.ts";
import { request_frame } from "@/kernel/sched.ts";

export async function read_asset(load: u32, signal: AbortSignal): Promise<U8Array> {
  const key = loadKey(load);
  if (key === null) throw new Error(`load ${load} names no asset`);
  const res = await originGet(assetUrl(key), signal);
  if (!res.ok) throw new Error(`${key} ${res.status}`);
  return res;
}

/** Tells the kernel the read under `key` did not arrive: it asks again in
 * its own time. */
function refuse(key: u64): void {
  CORE_DOORBELL.send(0, Command.LOAD_FAILED, encode_load_failed({ key }));
  request_frame();
}

/** Runs one read the scheduler started, and reports what became of it under
 * the key it was started with: `take` reads what arrived and gives back its
 * report, made only if the read is still this one's to report. */
async function read(key: u64, load: u32, take: (res: Response) => Promise<() => void>): Promise<void> {
  // A stop is the scheduler's own doing: it took the credit for something
  // the viewer is more waiting on, and it still wants this. The guest is told
  // the read did not arrive the moment it is stopped (fetch.ts begin_read).
  const { signal, own } = begin_read(key, () => refuse(key));
  try {
    const report = await take(await fetch_load(load, signal));
    if (!own()) return;
    report();
    request_frame();
  } catch (err) {
    if (!own()) return;
    console.error(`[kernel] asset: ${err instanceof Error ? err.message : String(err)}`);
    refuse(key);
  }
}

export function load_model(key: u64, load: u32): Promise<void> {
  return read(key, load, async (res) => {
    const bytes = new Uint8Array(await res.arrayBuffer());
    // A model is far past a ring frame: the host holds it, and the kernel
    // reads it in its turn.
    return () => CORE_DOORBELL.send(0, Command.LOAD_DONE, encode_load_done({ key, body: BODIES.hold(bytes), bytes: bytes.byteLength }));
  });
}

/** An image, as the kernel's renderer uploads it: the file's own pixels, top
 * row first, no color conversion and no premultiplied alpha. */
export function load_image(key: u64, load: u32): Promise<void> {
  return read(key, load, async (res) => {
    const handle = await holdImage(res);
    // An image that does not decode is refused, as a failed read is.
    return () => handle === null ? refuse(key) : CORE_DOORBELL.send(0, Command.LOAD_IMAGE, encode_load_image({ key, handle }));
  });
}

// @pxl/web/kernel/io/load.ts

/**
 * The models and images the scheduler starts a read for
 * (crates/pxl/web/kernel/src/io/load.rs): the kernel says a number, the shell
 * knows which asset that is and where it lives.
 *
 * A read of an asset is a read like any other — it carries the key the
 * scheduler handed out, it can be stopped by that key, and it spends one of
 * the same credits the pages do. What is different is what comes back: a
 * model is far past a ring frame, so the host holds its bytes and the kernel
 * reads them in its turn, and an image is decoded where the device is instead
 * (platform/web/image.ts on the web), because what the renderer uploads is
 * the file's own pixels.
 */

import type { u32, u64 } from "@xpute/core/abi/word.ts";
import { Command, encode_load_delete, encode_load_image, encode_load_update } from "@/kernel/abi/cmd.spec.ts";
import { loadKey } from "@/catalog/asset/load.ts";
import { originGet } from "@platform/origin.ts";
import { assetUrl } from "@/config/route.ts";
import { begin_read } from "@/kernel/io/fetch.ts";
import { CORE_DOORBELL } from "@/kernel/mem.ts";
import { hold_body } from "@/kernel/module.ts";
import { holdImage } from "@platform/image.ts";
import { request_frame } from "@/kernel/sched.ts";

/** The bytes a load names: the kernel says a number, the shell knows the URL. */
async function fetch_load(load: u32, signal: AbortSignal): Promise<Response> {
  const key = loadKey(load);
  if (key === null) throw new Error(`load ${load} names no asset`);
  const res = await originGet(assetUrl(key), signal);
  if (!res.ok) throw new Error(`${key} ${res.status}`);
  return res;
}

/** Tells the kernel the read under `key` did not arrive: it asks again in
 * its own time. */
function refuse(key: u64): void {
  CORE_DOORBELL.send(0, Command.LOAD_DELETE, encode_load_delete({ key }));
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
    return () => CORE_DOORBELL.send(0, Command.LOAD_UPDATE, encode_load_update({ key, body: hold_body(bytes), bytes: bytes.byteLength }));
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

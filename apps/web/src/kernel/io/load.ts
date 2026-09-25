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
 * reads them in its turn, and an image is decoded here instead, because what
 * the renderer uploads is the file's own pixels.
 */

import type { u32, u64 } from "@xpute/core/abi/word.ts";
import { Command, encode_load_delete, encode_load_image, encode_load_update } from "@/kernel/abi/cmd.spec.ts";
import { load_key } from "@/catalog/asset/load.ts";
import { asset_url } from "@/config/route.ts";
import { running } from "@/kernel/io/fetch.ts";
import { CORE_BUILD } from "@/kernel/ipc.ts";
import { hold_body } from "@/kernel/module.ts";
import { register_image } from "@/kernel/render/gpu.ts";
import { request_frame } from "@/kernel/sched.ts";

let next_image = 1;

/** The bytes a load names: the kernel says a number, the shell knows the URL. */
async function fetch_load(load: u32, signal: AbortSignal): Promise<Response> {
  const key = load_key(load);
  if (key === null) throw new Error(`load ${load} names no asset`);
  const res = await fetch(asset_url(key), { signal });
  if (!res.ok) throw new Error(`${key} ${res.status}`);
  return res;
}

/** Runs one read the scheduler started, and reports what became of it under
 * the key it was started with. */
async function read(key: u64, load: u32, take: (res: Response) => Promise<void>): Promise<void> {
  const controller = new AbortController();
  running.set(key, controller);
  try {
    await take(await fetch_load(load, controller.signal));
  } catch (err) {
    // A stop is the scheduler's own doing: it took the credit for something
    // the viewer is more waiting on, and it still wants this. Either way the
    // guest is told the read did not arrive and asks again in its own time.
    if (!controller.signal.aborted) console.error(`[kernel] asset: ${err instanceof Error ? err.message : String(err)}`);
    CORE_BUILD.send(0, Command.LOAD_DELETE, encode_load_delete({ key }));
  } finally {
    running.delete(key);
    request_frame();
  }
}

export function load_model(key: u64, load: u32): Promise<void> {
  return read(key, load, async (res) => {
    const bytes = new Uint8Array(await res.arrayBuffer());
    // A model is far past a ring frame: the host holds it, and the kernel
    // reads it in its turn.
    CORE_BUILD.send(0, Command.LOAD_UPDATE, encode_load_update({ key, body: hold_body(bytes), bytes: bytes.byteLength }));
  });
}

/** An image, as the kernel's renderer uploads it: the file's own pixels, top
 * row first, no colour conversion and no premultiplied alpha. */
export function load_image(key: u64, load: u32): Promise<void> {
  return read(key, load, async (res) => {
    const image = await createImageBitmap(await res.blob(), { colorSpaceConversion: "none", premultiplyAlpha: "none" });
    const handle = next_image++;
    register_image(handle, image);
    CORE_BUILD.send(0, Command.LOAD_IMAGE, encode_load_image({ key, handle }));
  });
}

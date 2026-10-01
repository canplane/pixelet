// @pxl/web/kernel/io/load.ts

/**
 * The assets the kernel reads (crates/pxl/web/kernel/src/io/load.rs): the
 * kernel says a number, the shell knows which asset that is and where it
 * lives.
 *
 * A model is a file below the assets' root, read whole like any other
 * (`read_model`, host.ts). An image is decoded where the device is instead
 * (platform/web/image.ts on the web), because what the renderer uploads is
 * the file's own pixels: the kernel asks for it by command, stops it by the
 * key it started it with, and is answered with the shell's handle.
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import type { u32, u64 } from "@xpute/core/abi/word.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { FileError } from "@xpute/runtime/ipc/sys.ts";
import { Command, encode_load_failed, encode_load_image } from "@/kernel/abi/cmd.spec.ts";
import { loadKey } from "@/catalog/asset/load.ts";
import { originGet } from "@platform/origin.ts";
import { assetUrl } from "@/config/route.ts";
import { CORE_DOORBELL } from "@/kernel/mem.ts";
import { holdImage } from "@platform/image.ts";
import { request_frame } from "@/kernel/sched.ts";

/** The bytes a load names: the kernel says a number, the shell knows the URL. */
async function fetch_load(load: u32, signal: AbortSignal): Promise<Response> {
  const key = loadKey(load);
  if (key === null) throw new FileError(Errno.ENOENT);
  const res = await originGet(assetUrl(key), signal);
  if (!res.ok) throw new FileError(res.status === 404 ? Errno.ENOENT : Errno.EACCES, res.status);
  return res;
}

/** A model's file, whole. */
export async function read_model(load: u32, signal: AbortSignal): Promise<U8Array> {
  return new Uint8Array(await (await fetch_load(load, signal)).arrayBuffer());
}

/** Every image read running, by the key the kernel started it under, with
 * what stops it. */
const running = new Map<bigint, AbortController>();

/** Tells the kernel the image under `key` did not arrive: it asks again in
 * its own time. */
function refuse(key: u64): void {
  CORE_DOORBELL.send(0, Command.LOAD_FAILED, encode_load_failed({ key }));
  request_frame();
}

/** An image, as the kernel's renderer uploads it: the file's own pixels, top
 * row first, no color conversion and no premultiplied alpha. */
export async function load_image(key: u64, load: u32): Promise<void> {
  const controller = new AbortController();
  running.set(key, controller);
  const own = () => running.get(key) === controller && running.delete(key);
  try {
    const handle = await holdImage(await fetch_load(load, controller.signal));
    if (!own()) return;
    // An image that does not decode is refused, as a failed read is.
    if (handle === null) refuse(key);
    else {
      CORE_DOORBELL.send(0, Command.LOAD_IMAGE, encode_load_image({ key, handle }));
      request_frame();
    }
  } catch (err) {
    if (!own()) return;
    console.error(`[kernel] asset: ${err instanceof Error ? err.message : String(err)}`);
    refuse(key);
  }
}

/** Stops the image read under `key`. The kernel still wants it, so it is
 * told at once that the read did not arrive, rather than when a transport
 * lets the read go — a device's fetch once left a stopped body unsettled for
 * good. */
export function cancel_load(key: u64): void {
  const controller = running.get(key);
  if (!controller) return;
  running.delete(key);
  controller.abort();
  refuse(key);
}

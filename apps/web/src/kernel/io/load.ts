// @pxl/web/kernel/io/load.ts

/**
 * The models and images the kernel asks for
 * (crates/pxl/web/kernel/src/io/load.rs): the kernel says a number, the shell
 * knows which asset that is and where it lives.
 *
 * A model is far past a ring frame, so the host holds its bytes and the
 * kernel reads them in its turn. An image is decoded here instead, because
 * what the renderer uploads is the file's own pixels.
 */

import { Cmd } from "@/kernel/abi/cmd.ts";
import { load_key } from "@/catalog/asset/load.ts";
import { asset_url } from "@/config/route.ts";
import { CORE_BUILD, packet } from "@/kernel/ipc.ts";
import { hold_body } from "@/kernel/module.ts";
import { register_image } from "@/kernel/render/gpu.ts";
import { request_frame } from "@/kernel/sched.ts";

let next_image = 1;

/** The bytes a load names: the kernel says a number, the shell knows the URL. */
async function fetch_load(load: number): Promise<Response> {
  const key = load_key(load);
  if (key === null) throw new Error(`load ${load} names no asset`);
  const res = await fetch(asset_url(key));
  if (!res.ok) throw new Error(`${key} ${res.status}`);
  return res;
}

export async function load_model(load: number): Promise<void> {
  try {
    const bytes = new Uint8Array(await (await fetch_load(load)).arrayBuffer());
    // A model is far past a ring frame: the host holds it, and the kernel
    // reads it in its turn.
    CORE_BUILD.send(0, Cmd.LOAD_UPDATE, packet((b) => b.u32(load).u32(hold_body(bytes)).u32(bytes.byteLength)));
  } catch (err) {
    console.error(`[kernel] gltf: ${err instanceof Error ? err.message : String(err)}`);
    CORE_BUILD.send(0, Cmd.LOAD_DELETE, packet((b) => b.u32(load)));
  }
  request_frame();
}

/** An image, as the kernel's renderer uploads it: the file's own pixels, top
 * row first, no colour conversion and no premultiplied alpha. */
export async function load_image(load: number): Promise<void> {
  try {
    const image = await createImageBitmap(await (await fetch_load(load)).blob(), { colorSpaceConversion: "none", premultiplyAlpha: "none" });
    const handle = next_image++;
    register_image(handle, image);
    CORE_BUILD.send(0, Cmd.LOAD_IMAGE, packet((b) => b.u32(load).u32(handle)));
  } catch (err) {
    console.error(`[kernel] image: ${err instanceof Error ? err.message : String(err)}`);
    CORE_BUILD.send(0, Cmd.LOAD_DELETE, packet((b) => b.u32(load)));
  }
  request_frame();
}

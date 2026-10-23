// @pxl/native/origin.ts

import { PLATFORM } from "./module.ts";

/** A device's app has no origin of its own. */
export const ORIGIN = "https://plei.me";

/** What the bake stores its objects as, asked for so they come as stored. */
const ZSTD = "zstd";

/** Rejects with a TypeError on a transport failure, as the page's fetch
 * does; Expo's FetchError covers both a failure and a stop. A body sent zstd
 * is decoded natively off this thread (PlatformModule `unpack`), as a browser
 * decodes its own, so no reader sees an encoding; a failed decode is a
 * transport failure. */
export async function originGet(route: string, signal?: AbortSignal): Promise<Response> {
  try {
    const res = await fetch(`${ORIGIN}${route}`, { signal, headers: { "accept-encoding": ZSTD } });
    if (res.headers.get("content-encoding") !== ZSTD) return res;
    const body = await PLATFORM.unpack(new Uint8Array(await res.arrayBuffer()));
    const headers = new Headers(res.headers);
    headers.delete("content-encoding");
    headers.delete("content-length");
    return new Response(body, { status: res.status, statusText: res.statusText, headers });
  } catch (err) {
    if (signal?.aborted) throw err;
    throw new TypeError(err instanceof Error ? err.message : String(err), { cause: err });
  }
}

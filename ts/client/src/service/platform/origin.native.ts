// @pxl/client/service/platform/origin.native.ts

import { td } from "@xpute/kit/codec/encoding.ts";

import { PLATFORM } from "./pxl-platform.native.ts";

/** A device's app has no origin of its own: it reads from the site's. */
export const REMOTE: boolean = true;

/** The encoding the bake stores its objects in. It is asked for, so that the
 * objects arrive as stored. */
const ZSTD = "zstd";

/** Rejects with a TypeError on a transport failure, as the page's fetch does,
 * where Expo's FetchError stands for both a failure and a stop. A body sent
 * zstd is decoded natively, off this thread (PlatformModule `unpack`), as a
 * browser decodes its own, so no reader ever sees an encoding. A failed decode
 * is a transport failure. */
export async function get(
  url: string,
  signal?: AbortSignal,
): Promise<Response> {
  try {
    const res = await fetch(url, {
      signal,
      headers: { "accept-encoding": ZSTD },
    });
    if (res.headers.get("content-encoding") !== ZSTD) return res;
    const body = await PLATFORM.unpack(new Uint8Array(await res.arrayBuffer()));
    const headers = new Headers(res.headers);
    headers.delete("content-encoding");
    headers.delete("content-length");
    const decoded = new Response(body, {
      status: res.status,
      statusText: res.statusText,
      headers,
    });
    // React Native's Response reads an ArrayBuffer's text one byte a character,
    // which breaks whatever UTF-8 is not ASCII.
    const text = () => Promise.resolve(td.decode(body));
    return Object.assign(decoded, {
      text,
      json: () => text().then(JSON.parse),
    });
  } catch (err) {
    if (signal?.aborted) throw err;
    throw new TypeError(err instanceof Error ? err.message : String(err), {
      cause: err,
    });
  }
}

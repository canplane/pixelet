// @pxl/client/service/platform/origin.native.ts

import { Platform } from "react-native";

import { PLATFORM } from "./pxl-platform.ts";

/** A device's app has no origin of its own: it reads from the site's. */
export const remote: boolean = true;

/** How many reads Expo's fetch runs at once. On Android, its OkHttp client
 * runs at most five calls to a host, and a sixth waits in OkHttp's own queue
 * (`Dispatcher` `maxRequestsPerHost`, which React Native's
 * OkHttpClientProvider leaves at its default). On iOS, URLSession caps
 * connections rather than requests, and the origin speaks HTTP/2, so one
 * connection carries every read. Nothing there caps the reads below the six
 * that the core's body slots are sized for (pxl-core sys/io/param.rs
 * `BODY_SLOTS`). */
export const credits = Platform.OS === "android" ? 5 : 6;

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
    return new Response(body, {
      status: res.status,
      statusText: res.statusText,
      headers,
    });
  } catch (err) {
    if (signal?.aborted) throw err;
    throw new TypeError(err instanceof Error ? err.message : String(err), {
      cause: err,
    });
  }
}

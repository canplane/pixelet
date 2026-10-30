// @pxl/native/origin.ts

import { Platform } from "react-native";

import { SITE_URL } from "@pxl/client/driver/namespace/route.spec.ts";
import { PLATFORM } from "./module.ts";

/** A device's app has no origin of its own: it reads from the site's. */
export const url = SITE_URL;

/** The reads Expo's fetch runs at once. On Android its OkHttp client runs at most
 * five calls a host, a sixth waiting in OkHttp's own queue (`Dispatcher`
 * `maxRequestsPerHost`, which React Native's OkHttpClientProvider leaves at its
 * default). On iOS URLSession caps connections, not requests, and the origin
 * speaks HTTP/2, one connection carrying every read, so nothing caps it below
 * the six the kernel's body slots are sized by (pxl-kernel sys/io/param.rs
 * `BODY_SLOTS`). */
export const credits = Platform.OS === "android" ? 5 : 6;

/** What the bake stores its objects as, asked for so they come as stored. */
const ZSTD = "zstd";

/** Rejects with a TypeError on a transport failure, as the page's fetch
 * does; Expo's FetchError covers both a failure and a stop. A body sent zstd
 * is decoded natively off this thread (PlatformModule `unpack`), as a browser
 * decodes its own, so no reader sees an encoding; a failed decode is a
 * transport failure. */
export async function get(
  route: string,
  signal?: AbortSignal,
): Promise<Response> {
  try {
    const res = await fetch(`${url}${route}`, {
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

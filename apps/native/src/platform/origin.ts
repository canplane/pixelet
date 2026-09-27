// @pxl/native/platform/origin.ts

/** Where the app's own routes are served from: the deployment, since a
 * device's app has no origin of its own. */
export const ORIGIN = "https://plei.me";

/** Reads one of the app's own routes, through the app's fetch — Expo's
 * (expo/fetch), which Expo installs as the global one on a device.
 *
 * A read the transport fails rejects with a TypeError, as the page's fetch
 * does, which is how the shell tells the network's failures from its own
 * (kernel/io/fetch.ts); Expo's says FetchError for both a failure and a stop. */
export async function originGet(route: string, signal?: AbortSignal): Promise<Response> {
  try {
    return await fetch(`${ORIGIN}${route}`, { signal });
  } catch (err) {
    if (signal?.aborted) throw err;
    throw new TypeError(err instanceof Error ? err.message : String(err), { cause: err });
  }
}

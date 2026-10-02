// @pxl/native/platform/origin.ts

/** A device's app has no origin of its own. */
export const ORIGIN = "https://plei.me";

/** Rejects with a TypeError on a transport failure, as the page's fetch
 * does; Expo's FetchError covers both a failure and a stop. */
export async function originGet(route: string, signal?: AbortSignal): Promise<Response> {
  try {
    return await fetch(`${ORIGIN}${route}`, { signal });
  } catch (err) {
    if (signal?.aborted) throw err;
    throw new TypeError(err instanceof Error ? err.message : String(err), { cause: err });
  }
}

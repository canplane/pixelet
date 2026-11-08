// @pxl/client/service/origin.ts

/** A page is served from the origin it reads from, so a route is fetched as
 * it stands. */
export const remote: boolean = false;

/** A browser opens six connections to a host over HTTP/1.1, and so runs six
 * reads at once there. A fetch past them waits in the browser's pool, which
 * knows nothing of what the core wants first. */
export const credits = 6;

export function get(url: string, signal?: AbortSignal): Promise<Response> {
  return fetch(url, { signal });
}

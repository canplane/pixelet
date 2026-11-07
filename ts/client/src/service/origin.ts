// @pxl/client/service/origin.ts

/** A page is served from the origin it reads, so a route stands as it is. */
export const remote: boolean = false;

/** A browser's six connections a host over HTTP/1.1, the reads it runs at once there; past them a fetch waits in the browser's pool, which knows nothing of what the core wants first. */
export const credits = 6;

export function get(url: string, signal?: AbortSignal): Promise<Response> {
  return fetch(url, { signal });
}

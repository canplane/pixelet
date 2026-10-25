// @pxl/web/origin.ts

export const url: string = "";

/** A browser's six connections a host over HTTP/1.1, the reads it runs at once there; past them a fetch waits in the browser's pool, which knows nothing of what the kernel wants first. */
export const credits = 6;

export function get(route: string, signal?: AbortSignal): Promise<Response> {
  return fetch(route, { signal });
}

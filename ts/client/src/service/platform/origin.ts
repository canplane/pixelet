// @pxl/client/service/platform/origin.ts

/** A page is served from the origin it reads from, so a route is fetched as
 * it stands. */
export const remote: boolean = false;

export function get(url: string, signal?: AbortSignal): Promise<Response> {
  return fetch(url, { signal });
}

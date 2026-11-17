// @pxl/client/service/platform/origin.web.ts

/** The web app is served from the origin it reads from, so a route is
 * fetched as it stands. The platform says only whether the origin is remote;
 * the site's address is service/origin's (route.ts `href`). */
export const REMOTE: boolean = false;

export function get(url: string, signal?: AbortSignal): Promise<Response> {
  return fetch(url, { signal });
}

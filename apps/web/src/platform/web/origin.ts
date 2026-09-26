// @pxl/web/platform/web/origin.ts

/** Where the app's own routes are served from, as a prefix to them: nothing
 * on the web, where the page and its routes share one origin. A device's app
 * has no origin of its own and names the deployment. */
export const ORIGIN: string = "";

/** Reads one of the app's own routes: the page's fetch. */
export function origin_get(route: string, signal?: AbortSignal): Promise<Response> {
  return fetch(route, { signal });
}

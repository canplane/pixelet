// @pxl/web/origin.ts

export const url: string = "";

export function get(route: string, signal?: AbortSignal): Promise<Response> {
  return fetch(route, { signal });
}

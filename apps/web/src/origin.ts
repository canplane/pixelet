// @pxl/web/origin.ts

export const ORIGIN: string = "";

export function originGet(route: string, signal?: AbortSignal): Promise<Response> {
  return fetch(route, { signal });
}

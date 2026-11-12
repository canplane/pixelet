// @pxl/client/service/platform/log.ts

/** Writes a line where the page's lines go. A build made for measuring also
 * keeps it in `globalThis.lines`, for the tool that drives the page
 * (script/perf/). */

export function log(line: string): void {
  if (__PERF_DRIVE__) ((globalThis as { lines?: string[] }).lines ??= []).push(line);
  console.log(line);
}

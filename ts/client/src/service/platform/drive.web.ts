// @pxl/client/service/platform/drive.web.ts

/** Tells `line` to the tool that drives a build made for measuring
 * (script/perf/): kept in `globalThis.lines`, which the tool reads from the
 * page, as no driver of Safari's reads its console. */
export function tell_drive(line: string): void {
  ((globalThis as { lines?: string[] }).lines ??= []).push(line);
}

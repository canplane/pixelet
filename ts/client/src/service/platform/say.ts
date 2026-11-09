// @pxl/client/service/platform/say.ts

/** The lines said so far, where a tool driving the page reads them
 * (script/perf/). */
const said: string[] = [];
(globalThis as { said?: string[] }).said = said;

/** Writes a line where a person running the page reads it. */
export function say(line: string): void {
  said.push(line);
  console.log(line);
}

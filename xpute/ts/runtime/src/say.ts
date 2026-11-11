// @xpute/runtime/say.ts

/**
 * Writes a line where a page's lines go, as a native runtime writes one to
 * its process's output (xpute-runtime native/say.rs) and a device's UI with
 * `XPUTE.say`. The page's runtime says its own lines here, such as a core's
 * stop or a turn slower than the program asked (runtime.ts `Config`), and so
 * does a device that runs beside the core. The UI's thread writes each
 * line to the console, and keeps it in `globalThis.said` where the page made
 * that list for a tool driving it to read (`keep_said`). A worker has no page:
 * once its runtime is attached to the UI (remote.ts `attach`), its lines go up
 * the port and are said at the UI's end, and before that only to its console.
 */

type Said = { said?: string[] };
let up: ((line: string) => void) | null = null;

export function say(line: string): void {
  if (up) return up(line);
  (globalThis as Said).said?.push(line);
  console.log(line);
}

/** Keeps the page's lines from now on, for a tool driving the page. */
export function keep_said(): void {
  (globalThis as Said).said ??= [];
}

/** Sends this thread's lines through `send` from now on. */
export function say_through(send: (line: string) => void): void {
  up = send;
}

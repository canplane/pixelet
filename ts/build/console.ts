// @pxl/build/console.ts

/**
 * The dev server's half of a device's remote console (the app's
 * remote-console.ts): what a page posts to `/__console` is printed where the
 * server's own output goes and appended to `log`, so a phone on a tunnel is read
 * without a debugger attached. Each dev server mounts it at that path: Metro's
 * (the app's metro.config.ts) and the shader probe's (tool/shader).
 */

import { appendFileSync } from "node:fs";
import type { IncomingMessage, ServerResponse } from "node:http";

export const CONSOLE_PATH = "/__console";

/** The bytes of one post taken. Past them the post is cut after its last
 * whole line, and the cut is printed as a line of its own. */
const POST_MAX = 1 << 16;

export function consoleHandler(
  log: string,
): (req: IncomingMessage, res: ServerResponse) => void {
  return (req, res) => {
    if (req.method !== "POST") {
      res.statusCode = 405;
      res.end();
      return;
    }
    const chunks: Buffer[] = [];
    let kept = 0;
    let past = 0;
    req.on("data", (c: Buffer) => {
      const take = Math.min(POST_MAX - kept, c.length);
      if (take > 0) chunks.push(c.subarray(0, take));
      kept += take;
      past += c.length - take;
    });
    req.on("end", () => {
      let bytes = Buffer.concat(chunks);
      let cut = "";
      if (past > 0) {
        const whole = bytes.lastIndexOf(0x0a) + 1;
        cut = `[console] a post past ${POST_MAX} bytes: its last ${bytes.length - whole + past} bytes cut`;
        bytes = bytes.subarray(0, whole);
      }
      const lines = [...bytes.toString().split("\n"), cut].filter(Boolean);
      for (const line of lines) console.log(line);
      if (lines.length > 0) {
        appendFileSync(
          log,
          lines.map((l) => `${new Date().toISOString()} ${l}\n`).join(""),
        );
      }
      res.statusCode = 204;
      res.end();
    });
  };
}

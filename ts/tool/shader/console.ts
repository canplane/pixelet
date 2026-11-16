// @pxl/shader/console.ts

/**
 * The probe's dev server's half of a device's remote console
 * (remote-console.ts). What the page posts to `/__console` is printed where
 * the server's own output goes and appended to `log`, so that a phone on a
 * tunnel can be read without a debugger attached. vite.config.ts mounts it at
 * that path.
 */

import { appendFileSync } from "node:fs";
import type { IncomingMessage, ServerResponse } from "node:http";

export const CONSOLE_PATH = "/__console";

/** How many bytes of one post are taken. A longer post is cut after its last
 * whole line, and the cut is reported as a line of its own. */
const POST_MAX = 1 << 16;

export function console_handler(
  log: string,
): (req: IncomingMessage, res: ServerResponse) => void {
  return (req, res) => {
    if (req.method !== "POST") {
      res.statusCode = 405;
      res.end();
      return;
    }
    const chunks: Buffer[] = [];
    let size = 0;
    let past = 0;
    req.on("data", (c: Buffer) => {
      const take = Math.min(POST_MAX - size, c.length);
      if (take > 0) chunks.push(c.subarray(0, take));
      size += take;
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

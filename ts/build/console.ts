// @pxl/build/console.ts

/**
 * The dev server's half of a device's remote console (the app's
 * remote-console.ts): what a page posts to `/__console` is printed where the
 * server's own output goes and appended to `log`, so a phone on a tunnel is read
 * without a debugger attached. Each dev server mounts it at that path.
 */

import { appendFileSync } from "node:fs";
import type { IncomingMessage, ServerResponse } from "node:http";

export const CONSOLE_PATH = "/__console";

export function consoleHandler(
  log: string,
): (req: IncomingMessage, res: ServerResponse) => void {
  return (req, res) => {
    if (req.method !== "POST") {
      res.statusCode = 405;
      res.end();
      return;
    }
    let body = "";
    req.on("data", (c: Buffer) => {
      if (body.length < 1 << 16) body += c.toString();
    });
    req.on("end", () => {
      const lines = body.split("\n").filter(Boolean);
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

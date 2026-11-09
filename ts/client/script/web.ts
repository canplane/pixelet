// @pxl/client/script/web.ts

/**
 * What the page needs besides what Expo's web build makes (app.config.ts):
 *
 *   web.ts html    writes public/index.html, the template Expo fills, from
 *                  index.html. It puts in the site's URL (`%SITE_URL%`, for
 *                  link previews; spec/spec/route.json `origins`) and the
 *                  UI's ground (`%GROUND%`, for the first paint), and takes
 *                  out the comments, which are only for the source.
 *   web.ts stamp   writes dist/build.json, which says what the deployment
 *                  is, so that a checkout learns the number being served
 *                  without a credential (ts/build/stamp.ts).
 *   web.ts serve   serves dist/, the export a deploy publishes, on
 *                  PXL_PREVIEW_PORT (4173), forwarding the same routes a dev
 *                  server forwards (proxy.ts). An unknown path is answered
 *                  with the page, as the Worker answers it.
 */

import * as fs from "node:fs";
import * as http from "node:http";
import * as path from "node:path";
import process from "node:process";

import { stamp } from "@pxl/build/stamp.ts";
import { SITE_URL } from "@pxl/client/service/origin/route.spec.ts";
import { GROUND } from "@pxl/client/ui/shared/component/ground.ts";
import { forward, route_target } from "./proxy.ts";

const APP = path.dirname(import.meta.dirname);

function html(): void {
  const source = fs.readFileSync(path.join(APP, "index.html"), "utf8");
  const page = source
    .replace(/[ \t]*<!--[\s\S]*?-->[ \t]*\n?/g, "")
    .replaceAll("%SITE_URL%", SITE_URL)
    .replaceAll("%GROUND%", GROUND);
  fs.mkdirSync(path.join(APP, "public"), { recursive: true });
  fs.writeFileSync(path.join(APP, "public/index.html"), page);
}

function build_json(): void {
  const { official, id, time } = stamp();
  fs.writeFileSync(
    path.join(APP, "dist/build.json"),
    JSON.stringify({ number: official, id, time }) + "\n",
  );
}

const TYPES: Record<string, string> = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript",
  ".css": "text/css",
  ".json": "application/json",
  ".webmanifest": "application/manifest+json",
  ".wasm": "application/wasm",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".ico": "image/x-icon",
};

function serve(): void {
  const dist = path.join(APP, "dist");
  const port = Number(process.env.PXL_PREVIEW_PORT || 4173);
  http
    .createServer((req, res) => {
      const url = req.url ?? "/";
      const target = route_target(url);
      if (target) return forward(target, req, res);
      const name = path.normalize(decodeURIComponent(url.split("?")[0]));
      let file = path.join(dist, name);
      if (
        !file.startsWith(dist) ||
        !fs.statSync(file, { throwIfNoEntry: false })?.isFile()
      ) {
        file = path.join(dist, "index.html");
      }
      res.writeHead(200, {
        "content-type": TYPES[path.extname(file)] ?? "application/octet-stream",
      });
      fs.createReadStream(file).pipe(res);
    })
    .listen(port, () => console.log(`[web] dist/ on http://localhost:${port}`));
}

const what = process.argv[2];
if (what === "html") html();
else if (what === "stamp") build_json();
else if (what === "serve") serve();
else throw new Error(`web.ts: no step ${what ?? ""}; html, stamp or serve`);

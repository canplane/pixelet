// tool/ingest/asset/render_brand.ts
//
// Renders the app's icons and link-preview image from the brand sources in
// asset/brand — the world-cube symbol (a transparent 1024² PNG) and the
// 1200×630 preview (kept as PNG; it was delivered as a 10-bit AVIF, which
// sharp's libheif does not decode) — into apps/web/public, which the deploy
// ships as static assets. Re-runnable; the outputs are committed, so a
// changed source is followed by running this and committing what it wrote.
//
//   deno run -A tool/ingest/asset/render_brand.ts
//
// Sizes: apple-touch-icon 180 (iOS paints transparency black, so it is
// flattened onto the preview's own sky and inset a little); the manifest's
// 192 and 512 `any` icons transparent; a 512 `maskable` icon flattened and
// inset by the safe zone; an ICO of 16/32/48 as PNG entries, which every
// browser since Vista reads; and the preview re-encoded as JPEG for the
// crawlers.

import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
import sharp from "npm:sharp@0.33";

const ROOT = fileURLToPath(new URL("../../../", import.meta.url));
const SYMBOL = resolve(ROOT, "asset/brand/symbol.png");
const PREVIEW = resolve(ROOT, "asset/brand/og.png");
const OUT = resolve(ROOT, "apps/web/public");

/** The preview's sky, the ground an opaque icon sits on. */
const SKY = { r: 0x3e, g: 0xc8, b: 0xf5, alpha: 1 };
const CLEAR = { r: 0, g: 0, b: 0, alpha: 0 };

/** The symbol with its transparent margin cut away, so a size is the cube's
 * own size: with the margin kept, a 16-pixel favicon drew a 12-pixel cube. */
function symbol() {
  return sharp(SYMBOL).trim({ threshold: 8 });
}

async function icon(size: number, out: string, opts: { flatten?: boolean; pad?: number } = {}): Promise<void> {
  const inner = Math.round(size * (1 - (opts.pad ?? 0)));
  const symbol_png = await symbol().resize(inner, inner, { fit: "contain", background: CLEAR }).png().toBuffer();
  await sharp({ create: { width: size, height: size, channels: 4, background: opts.flatten ? SKY : CLEAR } })
    .composite([{ input: symbol_png, gravity: "centre" }])
    .png()
    .toFile(resolve(OUT, out));
}

async function ico(sizes: number[], out: string): Promise<void> {
  const entries: Uint8Array[] = [];
  for (const s of sizes) entries.push(new Uint8Array(await symbol().resize(s, s, { fit: "contain", background: CLEAR }).png().toBuffer()));
  const header = new Uint8Array(6 + 16 * sizes.length);
  const dv = new DataView(header.buffer);
  dv.setUint16(2, 1, true);
  dv.setUint16(4, sizes.length, true);
  let offset = header.length;
  sizes.forEach((s, i) => {
    const e = 6 + i * 16;
    header[e] = s;
    header[e + 1] = s;
    dv.setUint16(e + 4, 1, true);
    dv.setUint16(e + 6, 32, true);
    dv.setUint32(e + 8, entries[i].length, true);
    dv.setUint32(e + 12, offset, true);
    offset += entries[i].length;
  });
  const file = new Uint8Array(offset);
  file.set(header);
  let at = header.length;
  for (const entry of entries) {
    file.set(entry, at);
    at += entry.length;
  }
  await Deno.writeFile(resolve(OUT, out), file);
}

await icon(180, "favicon/apple-touch-icon.png", { flatten: true, pad: 0.16 });
await icon(192, "favicon/web-app-manifest-192x192.png", { pad: 0.06 });
await icon(512, "favicon/web-app-manifest-512x512.png", { pad: 0.06 });
await icon(512, "favicon/web-app-manifest-512x512-maskable.png", { flatten: true, pad: 0.24 });
await ico([16, 32, 48], "favicon/favicon.ico");
await sharp(PREVIEW).jpeg({ quality: 88, mozjpeg: true }).toFile(resolve(OUT, "og-image.jpg"));
console.log("[brand] icons and og-image written to apps/web/public");

#!/usr/bin/env -S deno run -A
// The night sky's stars, baked from the Yale Bright Star Catalog into the
// eight bytes a star the renderer reads (pxl-render's gpu/stars.rs).
//
//   deno run -A script/bake-stars.ts                    # fetch the catalog from CDS
//   deno run -A script/bake-stars.ts catalog.gz         # or read one already here
//
// The catalog is Hoffleit and Warren's fifth revised edition (1991), every
// star to about magnitude 6.5, which is what an eye sees on a dark night:
// CDS publishes it as V/50, a fixed-width table whose ReadMe gives the
// columns read below. It is a public catalog, credited here and where it is
// drawn rather than in THIRD_PARTY_NOTICES.md, which covers copied source.
//
// A star is two little-endian words: its right ascension as a u16 of a turn
// and its declination as an i16 of a quarter turn, then its visual magnitude
// and its B−V color as i16 thousandths. The quantization is 20″ in right
// ascension and 10″ in declination, under a pixel at any field of view the
// camera has. The few entries with no position — novae and objects the
// catalog has retired — are left out; a star with no color index takes
// the Sun's. Brightest first, so a prefix of the file is the sky to a
// magnitude.

const SOURCE = "https://cdsarc.cds.unistra.fr/ftp/V/50/catalog.gz";
const OUT = new URL("../crates/pxl/web/render/src/frame/shader/bright-stars.bin", import.meta.url);
/// The Sun's B−V, for a star the catalog gives no color.
const SOLAR_BV = 0.65;

async function catalog(path: string | undefined): Promise<string> {
  const packed = path ? await Deno.readFile(path) : new Uint8Array(await (await fetch(SOURCE)).arrayBuffer());
  const text = new Blob([packed]).stream().pipeThrough(new DecompressionStream("gzip"));
  return await new Response(text).text();
}

interface Star {
  ra: number;
  dec: number;
  vmag: number;
  bv: number;
}

/// A line's star, by the ReadMe's byte columns (1-based, inclusive); none
/// where it has no position or no magnitude.
function star(line: string): Star | null {
  const at = (from: number, to: number) => line.slice(from - 1, to).trim();
  const [rah, ram, ras] = [at(76, 77), at(78, 79), at(80, 83)];
  const [sign, ded, dem, des] = [at(84, 84), at(85, 86), at(87, 88), at(89, 90)];
  const vmag = at(103, 107);
  if (rah === "" || ded === "" || vmag === "") return null;
  const ra = (Number(rah) + Number(ram) / 60 + Number(ras) / 3600) * 15;
  const dec = (sign === "-" ? -1 : 1) * (Number(ded) + Number(dem) / 60 + Number(des) / 3600);
  const bv = at(110, 114);
  return { ra, dec, vmag: Number(vmag), bv: bv === "" ? SOLAR_BV : Number(bv) };
}

const stars = (await catalog(Deno.args[0])).split("\n").map(star).filter((s): s is Star => s !== null);
stars.sort((a, b) => a.vmag - b.vmag);

const out = new DataView(new ArrayBuffer(stars.length * 8));
stars.forEach((s, i) => {
  out.setUint16(i * 8, Math.round((s.ra / 360) * 65536) % 65536, true);
  out.setInt16(i * 8 + 2, Math.round((s.dec / 90) * 32767), true);
  out.setInt16(i * 8 + 4, Math.round(s.vmag * 1000), true);
  out.setInt16(i * 8 + 6, Math.round(s.bv * 1000), true);
});
await Deno.writeFile(OUT, new Uint8Array(out.buffer));
console.log(`[bake-stars] ${stars.length} stars, magnitude ${stars[0].vmag} to ${stars.at(-1)!.vmag} -> ${OUT.pathname}`);

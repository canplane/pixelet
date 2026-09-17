// script/locale.ts
//
// Every cube and decor the catalogs give out has a name in every locale, and
// every name in a locale is one the catalogs still give out.
//
// The rest of the chain from a stored number to what a person reads is held
// by something already: spec.ts writes the numbering from cube_type.json,
// cell/type.ts throws on a colour keyed by a type no channel has, and a
// texture the pack lacks falls back to the cube's own colour. The last link
// is the only silent one — i18next renders a missing key as the key itself,
// so a catalog entry nobody translated reads as `cubes.land.bare` on screen
// and a translation whose type was renamed or dropped simply stops being
// reached. This is that link.

import { dirname, join } from "@std/path";

import { decor_ids, placeable_cube_id, placeable_cubes } from "@pxl/shared/spec/cell/mod.ts";

const root = dirname(dirname(new URL(import.meta.url).pathname));
const LOCALES = ["en", "ko"];

type Catalogue = { [key: string]: string | Catalogue };

const problems: string[] = [];

/** Every leaf of a branch as a dotted path, or none where the branch is
 * missing entirely — which the caller reports as every key missing. */
function leaves(node: Catalogue[string] | undefined, prefix = ""): string[] {
  if (typeof node !== "object" || node === null) return prefix === "" ? [] : [prefix];
  return Object.entries(node).flatMap(([key, value]) => leaves(value, prefix === "" ? key : `${prefix}.${key}`));
}

/** What a branch holds against what the catalog gives out. */
function branch(locale: string, doc: Catalogue, name: string, want: string[]): void {
  const have = new Set(leaves(doc[name]));
  for (const key of want) {
    if (!have.has(key)) problems.push(`${locale}.json: ${name}.${key} has no name — the catalog gives out ${key}`);
  }
  for (const key of have) {
    if (!want.includes(key)) problems.push(`${locale}.json: ${name}.${key} names nothing the catalog gives out`);
  }
}

const cube_ids = placeable_cubes.map(placeable_cube_id);
const channels = [...new Set(placeable_cubes.map((cube) => cube.channel))];

for (const locale of LOCALES) {
  const doc: Catalogue = JSON.parse(await Deno.readTextFile(join(root, `apps/web/src/locale/${locale}.json`)));
  branch(locale, doc, "cubes", cube_ids);
  branch(locale, doc, "decors", [...decor_ids]);
  // The drawer groups the catalog by channel, one heading each.
  const sections = doc["drawer_modal"];
  branch(locale, typeof sections === "object" ? sections : {}, "sections", channels);
}

if (problems.length > 0) {
  for (const p of problems) console.error(p);
  console.error(`locale: ${problems.length} problem${problems.length === 1 ? "" : "s"}`);
  Deno.exit(1);
}
console.log(`locale: every cube and decor is named in ${LOCALES.length} locales`);

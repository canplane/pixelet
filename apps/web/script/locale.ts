#!/usr/bin/env -S deno run -A
// Every cube and decor the catalogs give out has a name in every locale, and
// every name is still given out. i18next renders a missing key as the key
// itself, so this link is otherwise silent.

import { dirname, join } from "@std/path";

import { decorKinds, decorName, placeableCubeChannel, placeableCubeId, placeableCubes } from "../src/catalog/cell/mod.ts";

const web = dirname(dirname(new URL(import.meta.url).pathname));
const LOCALES = ["en", "ko"];

type Catalog = { [key: string]: string | Catalog };

const problems: string[] = [];

function leaves(node: Catalog[string] | undefined, prefix = ""): string[] {
  if (typeof node !== "object" || node === null) return prefix === "" ? [] : [prefix];
  return Object.entries(node).flatMap(([key, value]) => leaves(value, prefix === "" ? key : `${prefix}.${key}`));
}

function branch(locale: string, doc: Catalog, name: string, want: string[]): void {
  const have = new Set(leaves(doc[name]));
  for (const key of want) {
    if (!have.has(key)) problems.push(`${locale}.json: ${name}.${key} has no name — the catalog gives out ${key}`);
  }
  for (const key of have) {
    if (!want.includes(key)) problems.push(`${locale}.json: ${name}.${key} names nothing the catalog gives out`);
  }
}

const cubeIds = placeableCubes.map(placeableCubeId);
const channels = [...new Set(placeableCubes.map(placeableCubeChannel))];

for (const locale of LOCALES) {
  const doc: Catalog = JSON.parse(await Deno.readTextFile(join(web, `src/locale/${locale}.json`)));
  branch(locale, doc, "cubes", cubeIds);
  branch(locale, doc, "decors", decorKinds.map(decorName));
  const sections = doc["drawer_modal"];
  branch(locale, typeof sections === "object" ? sections : {}, "sections", channels);
}

if (problems.length > 0) {
  for (const p of problems) console.error(p);
  console.error(`locale: ${problems.length} problem${problems.length === 1 ? "" : "s"}`);
  Deno.exit(1);
}
console.log(`locale: every cube and decor is named in ${LOCALES.length} locales`);

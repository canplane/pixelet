#!/usr/bin/env -S deno run -A
// Every cube and decor the catalogs give out has its one English name, which
// every locale shows, and every section a name in every locale; every name is
// still given out. i18next renders a missing key as the key itself, so this
// link is otherwise silent.

import { dirname, join } from "@std/path";

import { decorKinds, decorName, decorSection, placeableCubeId, placeableCubes, placeableCubeSection } from "../src/catalog/cell/mod.ts";

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
const sections = [...new Set(placeableCubes.map(placeableCubeSection))];
const decorSections = [...new Set(decorKinds.map(decorSection))];

for (const locale of LOCALES) {
  const doc: Catalog = JSON.parse(await Deno.readTextFile(join(web, `src/locale/${locale}.json`)));
  branch(locale, doc, "cubes", locale === "en" ? cubeIds : []);
  branch(locale, doc, "decor", locale === "en" ? decorKinds.map(decorName) : []);
  const drawer = doc["drawer"];
  branch(locale, typeof drawer === "object" ? drawer : {}, "sections", sections);
  branch(locale, typeof drawer === "object" ? drawer : {}, "decor_sections", decorSections);
}

if (problems.length > 0) {
  for (const p of problems) console.error(p);
  console.error(`locale: ${problems.length} problem${problems.length === 1 ? "" : "s"}`);
  Deno.exit(1);
}
console.log(`locale: every cube and decor is named once, and every section in ${LOCALES.length} locales`);

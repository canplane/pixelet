#!/usr/bin/env -S deno run -A
// Every block and pet the catalogs give out has its one English name, which
// every locale shows, and every drawer section a name in every locale; every
// name is still given out. i18next renders a missing key as the key itself,
// so this link is otherwise silent.

import { dirname, join } from "@std/path";

import { allCubes, cubeId, cubeSection, decorIds, decorName, decorSection } from "../src/catalog/block/mod.ts";
import { groups } from "../src/feature/character/variant.spec.ts";

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

const cubeIds = allCubes.map(cubeId);
const sections = [...new Set(allCubes.map(cubeSection))];
const decorSections = [...new Set(decorIds.map(decorSection))];

const node = (doc: Catalog, name: string): Catalog => {
  const n = doc[name];
  return typeof n === "object" ? n : {};
};

for (const locale of LOCALES) {
  const doc: Catalog = JSON.parse(await Deno.readTextFile(join(web, `src/locale/${locale}.json`)));
  const en = locale === "en";
  branch(locale, doc, "blocks", en ? [...cubeIds.map((id) => `cube.${id}`), ...decorIds.map((k) => `decor.${decorName(k)}`), "marker"] : []);
  branch(locale, doc, "pets", en ? Object.keys(groups.pet.keys) : []);
  const drawerSections = node(node(doc, "drawer"), "sections");
  branch(locale, drawerSections, "cube", sections);
  branch(locale, drawerSections, "decor", decorSections);
  if (typeof drawerSections["marker"] !== "string") problems.push(`${locale}.json: drawer.sections.marker has no name`);
}

if (problems.length > 0) {
  for (const p of problems) console.error(p);
  console.error(`locale: ${problems.length} problem${problems.length === 1 ? "" : "s"}`);
  Deno.exit(1);
}
console.log(`locale: every block and pet is named once, and every section in ${LOCALES.length} locales`);

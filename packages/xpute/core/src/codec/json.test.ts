// @xpute/core/codec/json.test.ts

/**
 * The host's end of golden/codec/json.tsv. There is no json.ts: the host's
 * JSON is the platform's own, and xpute-core's codec/json.rs is the port of
 * it for the records' tags. This holds the platform to the same bytes the
 * port is held to, so the record is a claim about both ends and not about
 * the port alone.
 */

import { assertEquals } from "@std/assert";

import { Golden } from "@xpute/core/golden.ts";
import { bytes_to_hex } from "@xpute/core/codec/encoding.ts";

const utf8 = (hex: string): string => new TextDecoder().decode(Uint8Array.from(hex.match(/../g) ?? [], (b) => parseInt(b, 16)));
const hex = (s: string): string => bytes_to_hex(new TextEncoder().encode(s));

function list(v: Golden, base: string): string[] {
  const out: string[] = [];
  for (let j = 0; v.has(`${base}.${j}`); j++) out.push(v.s(`${base}.${j}`));
  return out;
}

/** A string map as the tags are one: an object whose every value is a
 * string, or nothing. */
function parse_str_map(text: string): Record<string, string> | null {
  let doc: unknown;
  try {
    doc = JSON.parse(text);
  } catch {
    return null;
  }
  if (doc === null || typeof doc !== "object" || Array.isArray(doc)) return null;
  return Object.values(doc).every((x) => typeof x === "string") ? doc as Record<string, string> : null;
}

Deno.test("the host's JSON writes and reads what the record holds", async () => {
  const v = await Golden.load("golden/codec/json.tsv");
  for (let k = 0; v.has(`stringify.${k}.json`); k++) {
    const keys = list(v, `stringify.${k}.keys`);
    const values = list(v, `stringify.${k}.values`);
    const map: Record<string, string | undefined> = {};
    keys.forEach((key, j) => map[utf8(key)] = values[j] === "undefined" ? undefined : utf8(values[j]));
    assertEquals(hex(JSON.stringify(map)), v.s(`stringify.${k}.json`), `stringify.${k}`);
  }
  for (let k = 0; v.has(`parse.${k}.text`); k++) {
    const map = parse_str_map(utf8(v.s(`parse.${k}.text`)));
    const got = map === null ? "refused" : Object.keys(map).length === 0 ? "empty" : Object.entries(map).map(([key, val]) => `${hex(key)}=${hex(val)}`).join(",");
    assertEquals(got, v.s(`parse.${k}.read`), `parse.${k}`);
  }
});

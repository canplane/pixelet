// @pxl/web/catalog/cell/type.ts

import type { u32 } from "@xpute/core/abi/word.ts";

/** A catalog colour, in both the forms anyone wants it: as written, for a
 * record a person reads, and packed, for everything that has to light or blend
 * with it. One table carries both because they are one colour — a second table
 * keyed the same way is how they come to disagree. */
export interface Swatch {
  hex: string;
  rgb: u32;
}

interface ColorJsonEntry {
  /** How the colour is written in the JSON, which is how a person picks one. */
  hex: string;
  /** The same colour as the one number it is, packed `0xRRGGBB`, worked out by
   * the generator so nothing here parses text to find a colour out. */
  rgb: u32;
  note?: string;
}
interface ColorJsonMeta {
  description?: string;
  source?: string;
}
interface ColorJsonFile {
  _meta?: ColorJsonMeta;
  [chKey: string]: ColorJsonMeta | Record<string, ColorJsonEntry> | undefined;
}

export function buildColorsFromJson<K extends string>(
  types: Record<K, Record<string, number>>,
  json: ColorJsonFile,
  seed: Record<number, Swatch | null>,
): Record<K, Record<number, Swatch | null>> {
  const out = {} as Record<K, Record<number, Swatch | null>>;

  (Object.keys(json) as (K | "_meta")[]).forEach((chKey) => {
    if (chKey === "_meta") return;

    const typesForCh = types[chKey];
    if (!typesForCh) {
      throw new Error(`color JSON references unknown channel "${chKey}"`);
    }

    const chOut: Record<number, Swatch | null> = { ...seed };
    const chEntries = json[chKey] as Record<string, ColorJsonEntry>;
    Object.entries(chEntries).forEach(([type_name, entry]) => {
      const typeId = typesForCh[type_name];
      if (typeId === undefined) {
        throw new Error(`color JSON references unknown type "${chKey}.${type_name}"`);
      }
      chOut[typeId] = { hex: entry.hex, rgb: entry.rgb };
    });

    out[chKey] = chOut;
  });

  return out;
}

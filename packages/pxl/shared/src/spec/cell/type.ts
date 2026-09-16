// @pxl/shared/spec/cell/type.ts

/** A catalog colour, in both the forms anyone wants it: as written, for a
 * record a person reads, and packed, for everything that has to light or blend
 * with it. One table carries both because they are one colour — a second table
 * keyed the same way is how they come to disagree. */
export type Swatch = { hex: string; rgb: number };

type ColorJsonEntry = {
  /** How the colour is written in the JSON, which is how a person picks one. */
  hex: string;
  /** The same colour as the one number it is, packed `0xRRGGBB`, worked out by
   * the generator so nothing here parses text to find a colour out. */
  rgb: number;
  note?: string;
};
type ColorJsonMeta = { description?: string; source?: string };
type ColorJsonFile = { _meta?: ColorJsonMeta; [ch_key: string]: ColorJsonMeta | Record<string, ColorJsonEntry> | undefined };

export function build_colors_from_json<K extends string>(
  types: Record<K, Record<string, number>>,
  json: ColorJsonFile,
  seed: Record<number, Swatch | null>,
): Record<K, Record<number, Swatch | null>> {
  const out = {} as Record<K, Record<number, Swatch | null>>;

  (Object.keys(json) as (K | "_meta")[]).forEach((ch_key) => {
    if (ch_key === "_meta") return;

    const types_for_ch = types[ch_key];
    if (!types_for_ch) {
      throw new Error(`color JSON references unknown channel "${ch_key}"`);
    }

    const ch_out: Record<number, Swatch | null> = { ...seed };
    const ch_entries = json[ch_key] as Record<string, ColorJsonEntry>;
    Object.entries(ch_entries).forEach(([type_name, entry]) => {
      const type_id = types_for_ch[type_name];
      if (type_id === undefined) {
        throw new Error(`color JSON references unknown type "${ch_key}.${type_name}"`);
      }
      ch_out[type_id] = { hex: entry.hex, rgb: entry.rgb };
    });

    out[ch_key] = ch_out;
  });

  return out;
}

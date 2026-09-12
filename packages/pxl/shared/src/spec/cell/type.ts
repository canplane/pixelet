// @pxl/shared/spec/cell/type.ts

type ColorJsonEntry = { hex: string; note?: string };
type ColorJsonMeta = { description?: string; source?: string };
type ColorJsonFile = { _meta?: ColorJsonMeta; [ch_key: string]: ColorJsonMeta | Record<string, ColorJsonEntry> | undefined };

export function build_colors_from_json<K extends string>(
  types: Record<K, Record<string, number>>,
  json: ColorJsonFile,
  seed: Record<number, string | null>,
): Record<K, Record<number, string | null>> {
  const out = {} as Record<K, Record<number, string | null>>;

  (Object.keys(json) as (K | "_meta")[]).forEach((ch_key) => {
    if (ch_key === "_meta") return;

    const types_for_ch = types[ch_key];
    if (!types_for_ch) {
      throw new Error(`color JSON references unknown channel "${ch_key}"`);
    }

    const ch_out: Record<number, string | null> = { ...seed };
    const ch_entries = json[ch_key] as Record<string, ColorJsonEntry>;
    Object.entries(ch_entries).forEach(([type_name, entry]) => {
      const type_id = types_for_ch[type_name];
      if (type_id === undefined) {
        throw new Error(`color JSON references unknown type "${ch_key}.${type_name}"`);
      }
      ch_out[type_id] = entry.hex;
    });

    out[ch_key] = ch_out;
  });

  return out;
}

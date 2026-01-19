// @pxl/render/terrain/shader_meta.ts

import type { u8 } from "@xpute/spec/lib/kit/word.ts";

import { terrain_colors, terrain_types } from "@pxl/spec/cell/terrain.ts";
import { GROUND_CH_KEYS, type GroundChannelKey } from "@pxl/bake/terrain/format.ts";

export const KEYS = <T extends object>(v: T): Array<keyof T> => Object.keys(v) as Array<keyof T>;

export const IS_VALID_HEX = (hex: string): boolean => /^#([A-Fa-f0-9]{3,4}|[A-Fa-f0-9]{6}|[A-Fa-f0-9]{8})$/.test(hex);

export const gen = {
  // shader local variable
  CH: (ch_key: GroundChannelKey): string => `ch_${ch_key}`,

  // texture uniform
  U_CH: (ch_key: GroundChannelKey): string => `u_ch_${ch_key}`,

  // color uniform
  U_COLOR: (ch_key: GroundChannelKey, type_key: string): string => `u_color_ch_${ch_key}_${type_key}`,

  FUNC_COLOR: (ch_key: GroundChannelKey): string => `get_color_ch_${ch_key}`,
};

export type TerrainColorMeta = {
  ch_key: GroundChannelKey;
  type_key: string;
  type_id: u8;
  uniform_name: string;
  hex_color: string;
};
export const get_color_meta = (): TerrainColorMeta[] => {
  const out: TerrainColorMeta[] = [];

  GROUND_CH_KEYS.forEach((ch_key) => {
    const types = terrain_types[ch_key];
    const colors = terrain_colors[ch_key];
    KEYS(types).forEach((type_key) => {
      const type_id = types[type_key];
      const hex_color = colors[type_id];
      if (hex_color && IS_VALID_HEX(hex_color)) {
        out.push({
          ch_key,
          type_key,
          type_id,
          uniform_name: gen.U_COLOR(ch_key, type_key),
          hex_color,
        });
      }
    });
  });

  return out;
};

export const build_color_mappings = (meta: TerrainColorMeta[]): {
  color_uniforms_decl: string;
  color_mapping_funcs: string;
} => {
  let color_uniforms_decl = meta
    .map((x) => `uniform vec3 ${x.uniform_name};`)
    .join("\n  ");
  if (color_uniforms_decl) color_uniforms_decl += "\n  ";

  let color_mapping_funcs = "";
  GROUND_CH_KEYS.forEach((ch_key) => {
    const cases = meta
      .filter((x) => x.ch_key === ch_key)
      .map((x) => `if (abs(type - ${x.type_id}.0) < 0.1) return ${x.uniform_name};`)
      .join("\n    ");
    if (cases) {
      color_mapping_funcs += `vec3 ${gen.FUNC_COLOR(ch_key)}(float type) { ${cases} return vec3(0.0); }\n`;
    } else {
      color_mapping_funcs += `vec3 ${gen.FUNC_COLOR(ch_key)}(float type) { return vec3(0.0); }\n`;
    }
  });

  return { color_uniforms_decl, color_mapping_funcs };
};

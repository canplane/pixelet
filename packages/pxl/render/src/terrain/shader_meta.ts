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

// // 1. 변수의 '설계도'를 정의 (여기가 유일한 진실의 원천)
// export const SHADER_VARS = {
//   U_COLOR: {
//     name: (ch: string, type: string) => `u_color_ch_${ch}_${type}`,
//     type: "vec3",
//     toGLSL: (hex: string) => hexToVec3(hex), // 값 변환 로직까지 한 세트
//   },
//   FUNC_COLOR: {
//     name: (ch: string) => `get_color_ch_${ch}`,
//   }
// } as const;

// // 2. 사용은 아주 직관적으로 (Namespace 느낌)
// const u_name = SHADER_VARS.U_COLOR.name("ground", "grass");
// const u_val  = SHADER_VARS.U_COLOR.toGLSL("#ff0000");

// // 3. (꿀팁) 코드젠 할 때 '선언문'도 자동화 가능
// const decl = `uniform ${SHADER_VARS.U_COLOR.type} ${u_name};`;

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

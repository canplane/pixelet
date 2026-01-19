// @pxl/web/modules/world/terrain/shader/mat.ts

import * as THREE from "three";

import type { f64, u32 } from "@xpute/spec/lib/kit/word.ts";

import { GROUND_CH_KEYS, type GroundChannelKey } from "@pxl/bake/terrain/format.ts";
import { create_frag, vert } from "@pxl/render/terrain/shader.ts";
import * as meta from "@pxl/render/terrain/shader_meta.ts";

const frag = create_frag();

export function create_mat(
  splatmaps: Record<GroundChannelKey, THREE.DataTexture>,
  heightmap: THREE.DataTexture,
  row_stride: u32,
  scale: f64,
): THREE.ShaderMaterial {
  const color_uniforms: Record<string, { value: THREE.Color }> = {};
  const color_meta = meta.get_color_meta();

  color_meta.forEach((x) => {
    color_uniforms[x.uniform_name] = {
      value: new THREE.Color(x.hex_color),
    };
  });

  const uniforms: Record<string, any> = {
    ...THREE.UniformsLib.fog,

    u_heightmap: { value: heightmap },
    u_row_stride: { value: row_stride },
    u_scale: { value: scale },

    u_lod_step: { value: 1 },

    u_edit_mask: { value: new THREE.DataTexture(new Uint8Array([0]), 1, 1) },
    u_border_opacity: { value: 0.018 },

    ...color_uniforms,
  };
  GROUND_CH_KEYS.forEach((ch_key) => {
    // e.g., u_ch_land: { value: splatmaps.land }
    uniforms[meta.gen.U_CH(ch_key)] = { value: splatmaps[ch_key] };
  });

  return new THREE.ShaderMaterial({
    glslVersion: THREE.GLSL3,
    uniforms,
    vertexShader: vert,
    fragmentShader: frag,
    transparent: false,
    depthWrite: true,
    depthTest: true,
    side: THREE.DoubleSide,
    fog: true,
    extensions: {
      derivatives: true,
    } as any,
  });
}

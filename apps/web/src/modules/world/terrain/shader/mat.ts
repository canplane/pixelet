// @pxl/web/modules/world/terrain/shader/mat.ts

import * as THREE from "three";

import type { f64, u32 } from "@xpute/core/kit/word.ts";

import { GROUND_CH_KEYS, type GroundChannelKey } from "@pxl/shared/bake/format.ts";
import { create_frag, depth_frag, depth_vert, vert } from "@/render/terrain/shader.ts";
import * as meta from "@/render/terrain/shader_meta.ts";
import { SUN_DIR } from "@/modules/world/render/sun_dir.ts";

const frag = create_frag();

export type TerrainMat = {
  mat: THREE.ShaderMaterial;
  /** Assign to mesh.customDepthMaterial — see shader.ts's depth_vert doc comment for why this exists. */
  depth_mat: THREE.ShaderMaterial;
};

export function create_mat(
  splatmaps: Record<GroundChannelKey, THREE.DataTexture>,
  heightmap: THREE.DataTexture,
  row_stride: u32,
  scale: f64,
): TerrainMat {
  const color_uniforms: Record<string, { value: THREE.Color }> = {};
  const color_meta = meta.get_color_meta();

  color_meta.forEach((x) => {
    color_uniforms[x.uniform_name] = {
      value: new THREE.Color(x.hex_color),
    };
  });

  // shared by reference (not spread-copied) between `mat` and `depth_mat`:
  // chunk.tsx's LOD useFrame only ever writes `mat.uniforms.u_lod_step.value`,
  // and relies on that single write reaching the depth material too since
  // it's the exact same {value:...} object under both materials' uniforms —
  // same pattern as u_light_dir/SUN_DIR below, just within one chunk instead
  // of across all of them.
  const core_uniforms = {
    u_heightmap: { value: heightmap },
    u_row_stride: { value: row_stride },
    u_scale: { value: scale },
    u_lod_step: { value: 1 },
    u_edit_mask: { value: new THREE.DataTexture(new Uint8Array([0]), 1, 1) },
    // LOD-boundary blend (see lod.ts's get_lod_guide + position_core in
    // shader.ts). 0/0 is the inert default: role 0 (finer) with t=0 always
    // passes its own visibility check, so a chunk outside any blend band
    // behaves exactly as it did before this existed. chunk.tsx only ever
    // touches these on the second (blend-partner) material it conditionally
    // creates, plus the primary's when a blend is active.
    u_blend_t: { value: 0 },
    u_blend_role: { value: 0 },
    // only read by the color fragment shader (depth_frag has no color
    // output to gate), but lives here anyway so depth_mat gets the same
    // `{value}` object for free via the shared-reference trick this comment
    // already documents — harmless for depth_mat to carry an unused
    // uniform. True by default; chunk.tsx syncs it from the debug store
    // right after each material is created, same live-update pattern as
    // u_lod_step/u_blend_t above (not a useMemo dep — toggling this is a
    // debug button click, not worth a full shader recompile per chunk).
    u_show_structures: { value: true },
  };

  const uniforms: Record<string, any> = {
    ...THREE.UniformsLib.fog,
    // `lights: true` below tells three to keep this material's light/shadow
    // uniforms refreshed every frame, but only for uniform keys that already
    // exist on the material — it assigns onto e.g. uniforms.ambientLightColor
    // .value rather than creating that key, so without this merge the
    // assignment throws ("Cannot set properties of undefined") the moment any
    // light is present. THREE.UniformsLib.lights is exactly the uniform group
    // its own lit materials (MeshStandardMaterial etc.) merge in for the same
    // reason — cloned so multiple chunk materials don't share one mutable
    // array/object graph.
    ...THREE.UniformsUtils.clone(THREE.UniformsLib.lights),

    ...core_uniforms,

    // shared across every chunk's material — see sun_dir.ts. environment.tsx
    // mutates this one Vector3 once a frame; every chunk reads the same
    // object, so the ground's shading tracks the real sun/theme instead of
    // a fixed baked-in direction.
    u_light_dir: { value: SUN_DIR },

    u_border_opacity: { value: 0.018 },

    ...color_uniforms,
  };
  GROUND_CH_KEYS.forEach((ch_key) => {
    // e.g., u_ch_land: { value: splatmaps.land }
    uniforms[meta.gen.U_CH(ch_key)] = { value: splatmaps[ch_key] };
  });

  const mat = new THREE.ShaderMaterial({
    glslVersion: THREE.GLSL3,
    uniforms,
    vertexShader: vert,
    fragmentShader: frag,
    transparent: false,
    depthWrite: true,
    depthTest: true,
    side: THREE.DoubleSide,
    fog: true,
    // turns on three's normal light/shadow uniform wiring for this material
    // (ambient/directional/shadow-map uniforms get bound automatically at
    // render time, the same machinery MeshStandardMaterial etc. use) — the
    // shader itself still does all its own shading by hand via u_light_dir;
    // this only exists so the #include<shadowmap_*> chunks in shader.ts have
    // real uniforms to read from. See shader.ts for the receiving half.
    lights: true,
    extensions: {
      derivatives: true,
    } as any,
  });

  const depth_mat = new THREE.ShaderMaterial({
    glslVersion: THREE.GLSL3,
    uniforms: core_uniforms,
    vertexShader: depth_vert,
    fragmentShader: depth_frag,
    side: THREE.DoubleSide,
  });

  return { mat, depth_mat };
}

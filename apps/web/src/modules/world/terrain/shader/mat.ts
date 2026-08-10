// @pxl/web/modules/world/terrain/shader/mat.ts

import * as THREE from "three";

import type { f64, u32 } from "@xpute/core/kit/word.ts";

import { GROUND_CH_KEYS, type GroundChannelKey } from "@pxl/shared/bake/format.ts";
import { create_frag, create_vert, depth_frag, depth_vert } from "@/render/terrain/shader.ts";
import * as meta from "@/render/terrain/shader_meta.ts";
import { SUN_DIR } from "@/modules/world/render/sun_dir.ts";
import { create_ground_layer_texture, load_real_ground_layer_images } from "./tex.ts";
import { GROUND_LAYER_IMAGE_MAP, GROUND_LAYER_SIDE_IMAGE_MAP } from "./ground_layer_images.ts";

const frag = create_frag();
const vert = create_vert();

// Shared across every chunk's material (unlike splatmaps/heightmap, which
// are per-chunk data) — one texture array, one layer per ground type,
// built exactly once at module load. See shader_meta.ts's get_layer_meta/
// build_layer_texture_data for why this can be a single flat-fill-per-layer
// array today and a real texture-pack image per layer later with no shader
// change, and tex.ts's create_ground_layer_texture for the texture itself.
const ground_layer_meta = meta.get_layer_meta();
const ground_layer_tex_data = meta.build_layer_texture_data(ground_layer_meta);
const ground_layers = create_ground_layer_texture(
  ground_layer_tex_data.data,
  ground_layer_tex_data.width,
  ground_layer_tex_data.height,
  ground_layer_tex_data.depth,
);
// Fire-and-forget: progressively upgrades ground_layers' flat-fill layers to
// real CC0 texture-pack images as each one finishes decoding — see its own
// doc comment (tex.ts) for why this is safe to kick off once at module load
// rather than gating anything on it.
load_real_ground_layer_images(ground_layers, ground_layer_meta, GROUND_LAYER_IMAGE_MAP, ground_layer_tex_data.width);

// Side-face counterpart of ground_layers — same layer indexing (ground_layer_
// meta is shared), same flat-fill-first/progressively-upgraded pattern, just
// backed by its own copy of the placeholder data (DataArrayTexture owns its
// buffer, can't share ground_layers' one) and loaded from
// GROUND_LAYER_SIDE_IMAGE_MAP instead — a type with no distinct side texture
// (most of them; see that map's own comment) resolves to the exact same
// image as the top array, so those layers end up visually identical between
// the two textures once both finish loading, same as before this existed.
const ground_layers_side = create_ground_layer_texture(
  new Uint8Array(ground_layer_tex_data.data),
  ground_layer_tex_data.width,
  ground_layer_tex_data.height,
  ground_layer_tex_data.depth,
);
load_real_ground_layer_images(ground_layers_side, ground_layer_meta, GROUND_LAYER_SIDE_IMAGE_MAP, ground_layer_tex_data.width);

export type TerrainMat = {
  mat: THREE.ShaderMaterial;
  /** Assign to mesh.customDepthMaterial — see shader.ts's depth_vert doc comment for why this exists. */
  depth_mat: THREE.ShaderMaterial;
};

export function create_mat(
  splatmaps: Record<GroundChannelKey, THREE.DataTexture>,
  heightmap: THREE.DataTexture,
  // per-chunk, real mip-blended color bake (shader_meta.ts's
  // decode_ground_rgba + tex.ts's create_ground_color_texture) — the
  // low-spec render path's own color source (see u_low_spec below); this is
  // what gives low-spec mode real cross-category color blending at coarse
  // LOD, which the shared ground_layers array can't reproduce on its own.
  ground_color: THREE.DataTexture,
  row_stride: u32,
  scale: f64,
  // per-chunk constant in [0,1) driving the LOD-blend visibility split (see
  // position_core's step 0 in shader.ts) — chunk.tsx computes this once from
  // the chunk's own page_idx+page_off and passes the *same* value into both
  // this call and its blend-partner material's call, so the two draws agree
  // on which one claims this chunk at any given u_blend_t.
  blend_r: f64,
): TerrainMat {
  // shared by reference (not spread-copied) between `mat` and `depth_mat`:
  // chunk.tsx's LOD useFrame only ever writes `mat.uniforms.u_lod_step.value`,
  // and relies on that single write reaching the depth material too since
  // it's the exact same {value:...} object under both materials' uniforms —
  // same pattern as u_light_dir/SUN_DIR below, just within one chunk instead
  // of across all of them.
  const core_uniforms = {
    u_heightmap: { value: heightmap },
    // per-chunk — see this function's own param comment. Only read by the
    // low-spec vertex shader (create_vert), but lives here anyway for the
    // same shared-reference reason u_show_structures below does.
    u_ground_color: { value: ground_color },
    // module-level shared texture (see its own comment above) — every
    // chunk's material points at the exact same THREE.DataArrayTexture.
    // Only read by the texture-pack fragment shader (create_frag) — low-spec
    // mode uses u_ground_color above instead (see u_low_spec's own comment).
    u_ground_layers: { value: ground_layers },
    // side-face counterpart of u_ground_layers above — see its own
    // module-level comment. create_frag's sample_ground_layer_color picks
    // between the two based on whether the fragment being shaded is a top
    // or side face.
    u_ground_layers_side: { value: ground_layers_side },
    // low-spec (default): sample u_ground_color (the real per-chunk mip-
    // blended bake) once per vertex, near-zero fragment cost, real
    // cross-category blending at coarse LOD — matches this material's
    // original vertex-color-esque design. Off: resolve+sample
    // u_ground_layers once per fragment instead, with a real tiling UV, so a
    // real (non-flat-fill) texture-pack image actually shows detail across
    // a cell's face — trading the blending away, since picking one winning
    // category per sample point is inherent to showing a real image rather
    // than a flat averaged color. See create_vert/create_frag's own
    // comments. A debug-menu toggle (dev_panel.tsx), not something that
    // varies per chunk.
    u_low_spec: { value: true },
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
    u_blend_r: { value: blend_r },
    // read by both create_vert (low-spec) and create_frag (texture-pack) —
    // whichever one is actually resolving the ground layer this frame needs
    // to know whether STRUCTURE is allowed to win. depth_vert/depth_frag
    // don't use it, but carry it anyway via the same shared-reference trick
    // this file already relies on elsewhere. True by default; chunk.tsx
    // syncs it from the debug store right after each material is created,
    // same live-update pattern as u_lod_step/u_blend_t above (not a
    // useMemo dep — toggling this is a debug button click, not worth a
    // full shader recompile per chunk).
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

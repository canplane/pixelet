// @pxl/web/modules/world/render/chunk_material.ts

/**
 * The baked chunk mesh's attribute names, injected shader, geometry builder,
 * and the one material every chunk mesh shares.
 *
 * Split out of chunk_stage.tsx so the imperative chunk node and the React
 * tree can both reach it without one importing the other, and so the
 * material can be a module singleton rather than a per-component useMemo.
 * There used to be one MeshStandardMaterial per chunk mesh — thousands of
 * them — even though `customProgramCacheKey` already forced them all to
 * share a single compiled program and nothing about them differs per chunk:
 * the two uniforms that vary (`u_low_spec`, `u_rim_intensity`) are global
 * settings, not per-chunk state.
 */

import * as THREE from "three";
import { invalidate } from "@react-three/fiber";

import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { light_cfg, rim_cfg } from "@/modules/world/environment/config.ts";
import { glsl_float, inject_rim_light_shader } from "@/modules/world/render/rim_light_shader.ts";
import { GROUND_LAYERS_SIDE, GROUND_LAYERS_TOP } from "@/render/tex_array.ts";
import { NO_LAYER } from "@/render/layer/pipeline/volume.ts";
import type { BuiltChunkMesh } from "@/render/layer/pipeline/chunk_bucket.ts";

// Per-vertex attribute names for the baked chunk mesh (chunk_mesher.ts) —
// replaces what used to be per-INSTANCE attributes read through
// instanceMatrix; every vertex now already carries its own real
// position/normal, so these are the only extra material/shading inputs the
// shader needs (see inject_chunk_shader's own doc comment).
const RGB_ATTR = "a_rgb";
const LAYER_TAPS_ATTR = "a_layer_taps";
export const VISIBLE_ATTR = "a_visible";
const LOCAL_POS_ATTR = "a_local_pos";
const BOX_SCALE_ATTR = "a_box_scale";

/**
 * Injected once per baked chunk-mesh material. Every vertex here is a real,
 * final position (chunk_mesher.ts bakes it directly — no instanceMatrix to
 * decompose), so this shader is actually SIMPLER than the instanced version
 * it replaced: `a_local_pos`/`a_box_scale` are baked per-vertex copies of
 * exactly what the old vertex shader used to reconstruct from `position` +
 * `instanceMatrix`'s diagonal, so the UV-tiling and bevel formulas below are
 * otherwise unchanged from before.
 */
function inject_chunk_shader(shader: {
  vertexShader: string;
  fragmentShader: string;
  uniforms: Record<string, THREE.IUniform>;
}): void {
  // Composes with the rest of this function regardless of call order (see
  // rim_light_shader.ts's own doc comment) — called first here purely by
  // convention, not because it has to run before the terrain-specific
  // replacements below.
  inject_rim_light_shader(shader);
  shader.uniforms.u_ground_layers_top = { value: GROUND_LAYERS_TOP };
  shader.uniforms.u_ground_layers_side = { value: GROUND_LAYERS_SIDE };
  shader.uniforms.u_low_spec = { value: true };
  shader.vertexShader = shader.vertexShader
    .replace(
      "#include <common>",
      `#include <common>
      attribute vec3 ${RGB_ATTR};
      attribute vec4 ${LAYER_TAPS_ATTR};
      attribute float ${VISIBLE_ATTR};
      attribute vec3 ${LOCAL_POS_ATTR};
      attribute vec3 ${BOX_SCALE_ATTR};
      varying vec3 v_rgb;
      varying vec4 v_layer_taps;
      varying float v_visible;
      varying vec2 v_box_uv;
      varying float v_is_side;
      varying float v_side_x_dominant;
      varying vec3 v_local_pos;
      varying vec3 v_box_scale;`,
    )
    .replace(
      "void main() {",
      `void main() {
        v_rgb = ${RGB_ATTR};
        v_layer_taps = ${LAYER_TAPS_ATTR};
        v_visible = ${VISIBLE_ATTR};
        v_is_side = (abs(normal.z) < 0.5) ? 1.0 : 0.0;
        v_local_pos = ${LOCAL_POS_ATTR};
        v_side_x_dominant = (abs(normal.x) > 0.5) ? 1.0 : 0.0;
        // Real-size tile UV, not the box's own raw 0..1 face UV: chunk_mesher.ts
        // bakes each box's own real cell-unit size straight into a_box_scale, so a
        // box spanning many cells — any coarse-LOD box, or same-material cells merged
        // at any LOD — was one whole texture tile stretched across the entire face instead
        // of repeating once per real cell. Most visible on patterned side art (assets/cell/
        // textures/terrain/side_*.png): its layered-strata look read as a flat smear
        // once stretched that far. a_local_pos is the box's own canonical [-0.5,0.5]
        // corner, so (a_local_pos.axis + 0.5) * a_box_scale.axis reconstructs a per-face
        // 0..N coordinate directly from it — sidesteps ever needing a per-face UV axis
        // convention. Side faces always put z (height) in the second component so
        // the strata art's up/down orientation stays consistent on all four side faces,
        // not flipped on two of them. GROUND_LAYERS_SIDE (tex_array.ts) is an untransformed
        // DataArrayTexture (flipY defaults false), so v=0 samples the source PNG's own top row
        // — (0.5 - a_local_pos.z), not (a_local_pos.z + 0.5), so the box's physical top
        // (a_local_pos.z = +0.5) lands on v=0 and actually gets the image's top edge instead
        // of its bottom (e.g. side_bare.png's grass cap rendering at a cliff's foot instead
        // of its crest). RepeatWrapping (tex_array.ts) turns >1 straight into tiling.
        v_box_scale = ${BOX_SCALE_ATTR};
        v_box_uv = v_is_side > 0.5
          ? (abs(normal.x) > 0.5
            ? vec2((${LOCAL_POS_ATTR}.y + 0.5) * ${BOX_SCALE_ATTR}.y, (0.5 - ${LOCAL_POS_ATTR}.z) * ${BOX_SCALE_ATTR}.z)
            : vec2((${LOCAL_POS_ATTR}.x + 0.5) * ${BOX_SCALE_ATTR}.x, (0.5 - ${LOCAL_POS_ATTR}.z) * ${BOX_SCALE_ATTR}.z))
          : vec2((${LOCAL_POS_ATTR}.x + 0.5) * ${BOX_SCALE_ATTR}.x, (${LOCAL_POS_ATTR}.y + 0.5) * ${BOX_SCALE_ATTR}.y);`,
    );
  shader.fragmentShader = shader.fragmentShader
    .replace(
      "#include <common>",
      `#include <common>
      varying vec3 v_rgb;
      varying vec4 v_layer_taps;
      varying float v_visible;
      varying vec2 v_box_uv;
      varying float v_is_side;
      varying float v_side_x_dominant;
      varying vec3 v_local_pos;
      varying vec3 v_box_scale;
      uniform sampler2DArray u_ground_layers_top;
      uniform sampler2DArray u_ground_layers_side;
      uniform bool u_low_spec;
      vec3 sample_layer_tap(float layer_idx, vec3 fallback_rgb) {
        if (layer_idx > ${NO_LAYER}.0 - 0.5) return fallback_rgb;
        return v_is_side > 0.5
          ? texture(u_ground_layers_side, vec3(v_box_uv, layer_idx)).rgb
          : texture(u_ground_layers_top, vec3(v_box_uv, layer_idx)).rgb;
      }
      // Per-cell edge crease — a static darken only, view-independent (no Fresnel/sheen
      // term), so it reads the same regardless of camera angle. Narrow width with a
      // hard-ish falloff (pow, not a wide smoothstep) keeps it a crisp seam rather than a
      // soft/hazy gradient.
      const float BEVEL_WORLD_WIDTH = 0.025;
      const float BEVEL_DARKEN = 0.78;
      float edge_factor() {
        vec3 dist_to_edge = (0.5 - abs(v_local_pos)) * v_box_scale;
        float d = v_is_side > 0.5
          ? (v_side_x_dominant > 0.5 ? min(dist_to_edge.y, dist_to_edge.z) : min(dist_to_edge.x, dist_to_edge.z))
          : min(dist_to_edge.x, dist_to_edge.y);
        return pow(1.0 - smoothstep(0.0, BEVEL_WORLD_WIDTH, d), 2.0);
      }
      vec3 apply_fake_bevel(vec3 color) {
        return color * mix(1.0, BEVEL_DARKEN, edge_factor());
      }`,
    )
    .replace("void main() {", `void main() { if (v_visible < 0.5) discard;`)
    .replace(
      "vec4 diffuseColor = vec4( diffuse, opacity );",
      `vec3 base_rgb = v_rgb;
      if (!u_low_spec) {
        base_rgb = (
          sample_layer_tap(v_layer_taps.x, base_rgb) +
          sample_layer_tap(v_layer_taps.y, base_rgb) +
          sample_layer_tap(v_layer_taps.z, base_rgb) +
          sample_layer_tap(v_layer_taps.w, base_rgb)
        ) * 0.25;
      }
      base_rgb = apply_fake_bevel(base_rgb);
      vec4 diffuseColor = vec4(diffuse * base_rgb, opacity);`,
    )
    .replace("vec3 totalEmissiveRadiance = emissive;", `vec3 totalEmissiveRadiance = base_rgb * ${glsl_float(light_cfg.FLOOR)};`);
}

/** Builds a fresh, real (non-cloned-from-a-shared-base) BufferGeometry
 * directly from one chunk's own baked vertex/index arrays — there's no
 * shared base to clone anymore, every chunk's geometry is entirely its own. */
export function create_geometry(built: BuiltChunkMesh): THREE.BufferGeometry {
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.BufferAttribute(built.position, 3));
  geo.setAttribute("normal", new THREE.BufferAttribute(built.normal, 3));
  geo.setAttribute(RGB_ATTR, new THREE.BufferAttribute(built.rgb, 3));
  geo.setAttribute(LAYER_TAPS_ATTR, new THREE.BufferAttribute(built.layer_taps, 4));
  geo.setAttribute(VISIBLE_ATTR, new THREE.BufferAttribute(built.visibility, 1));
  geo.setAttribute(LOCAL_POS_ATTR, new THREE.BufferAttribute(built.local_pos, 3));
  geo.setAttribute(BOX_SCALE_ATTR, new THREE.BufferAttribute(built.box_scale, 3));
  geo.setIndex(new THREE.BufferAttribute(built.index, 1));
  return geo;
}

function create_material(): THREE.MeshStandardMaterial {
  const material = new THREE.MeshStandardMaterial({ roughness: 1, metalness: 0 });
  // Shadow map records back-face depth instead of front-face — the receiver
  // then tests against the far side of the caster instead of its own lit
  // surface, so normalBias no longer has to fight self-shadow acne and can
  // shrink toward zero without the shadow visibly detaching from its own
  // caster (environment/config.ts's BIAS_TEXELS).
  material.shadowSide = THREE.BackSide;
  // Material.customProgramCacheKey() defaults to "" — WebGLPrograms' cache
  // key is built only from the defines/parameters three.js itself tracks,
  // never from onBeforeCompile's own identity or its string output. Terrain
  // and object cubes now bake into one merged mesh and compile the exact
  // same injected shader, so sharing one cached WebGLProgram across every
  // chunk mesh is correct.
  material.customProgramCacheKey = () => "chunk-mesh";
  material.onBeforeCompile = (shader) => {
    inject_chunk_shader(shader);
    shader.uniforms.u_low_spec.value = useSettingsStore.getState().low_spec;
    shader.uniforms.u_rim_intensity.value = useSettingsStore.getState().show_rim_light ? rim_cfg.INTENSITY : 0;
    material.userData.shader = shader;
  };
  return material;
}

/**
 * The one material every chunk mesh uses. Its two varying uniforms are
 * global settings, so they are pushed here on change rather than through a
 * per-mesh effect; `invalidate` wakes the demand loop, which a React state
 * change used to do implicitly by re-rendering.
 */
let shared_material: THREE.MeshStandardMaterial | null = null;

export function chunk_material(): THREE.MeshStandardMaterial {
  if (shared_material) return shared_material;
  shared_material = create_material();
  useSettingsStore.subscribe((state) => {
    const shader = shared_material?.userData.shader as { uniforms: Record<string, THREE.IUniform> } | undefined;
    if (!shader) return;
    shader.uniforms.u_low_spec.value = state.low_spec;
    shader.uniforms.u_rim_intensity.value = state.show_rim_light ? rim_cfg.INTENSITY : 0;
    invalidate();
  });
  return shared_material;
}

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

import { useDebugStore } from "@/app/settings/debug_store.ts";
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
const FACES_ATTR = "a_faces";

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
  shader.uniforms.u_edge_strength = { value: 0.3 };
  shader.uniforms.u_edge_width = { value: 0.03 };
  shader.vertexShader = shader.vertexShader
    .replace(
      "#include <common>",
      `#include <common>
      attribute vec3 ${RGB_ATTR};
      attribute vec4 ${LAYER_TAPS_ATTR};
      attribute float ${VISIBLE_ATTR};
      attribute vec3 ${LOCAL_POS_ATTR};
      attribute vec3 ${BOX_SCALE_ATTR};
      attribute float ${FACES_ATTR};
      varying vec3 v_rgb;
      varying vec4 v_layer_taps;
      varying float v_visible;
      varying vec2 v_box_uv;
      varying float v_is_side;
      varying float v_side_x_dominant;
      varying float v_side_sign;
      varying vec3 v_local_pos;
      varying vec3 v_box_scale;
      varying float v_faces;`,
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
        // Which way this side face points, for the diagonal lookup below —
        // vNormal is transformed by the time the fragment stage sees it.
        v_side_sign = (abs(normal.x) > 0.5) ? sign(normal.x) : sign(normal.y);
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
        v_faces = ${FACES_ATTR};
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
      varying float v_side_sign;
      varying vec3 v_local_pos;
      varying vec3 v_box_scale;
      varying float v_faces;
      uniform float u_edge_strength;
      uniform float u_edge_width;
      uniform sampler2DArray u_ground_layers_top;
      uniform sampler2DArray u_ground_layers_side;
      uniform bool u_low_spec;
      vec3 sample_layer_tap(float layer_idx, vec3 fallback_rgb) {
        if (layer_idx > ${NO_LAYER}.0 - 0.5) return fallback_rgb;
        return v_is_side > 0.5
          ? texture(u_ground_layers_side, vec3(v_box_uv, layer_idx)).rgb
          : texture(u_ground_layers_top, vec3(v_box_uv, layer_idx)).rgb;
      }
      // Edge shading, from one rule.
      //
      // For a face with outward normal n, its edge toward an in-plane
      // direction d is a crease when either the face toward d also
      // exists — nothing out that way, so the edge is a silhouette — or the
      // cell at (d + n) is solid, so the surface turns inward there. Convex
      // and concave, and every edge of every face is one, the other, both,
      // or neither.
      //
      // The bits come from chunk_mesher.ts, which answers the first half
      // already to decide whether to emit a face at all. Flat ground has no
      // creases and stays seamless, which is the whole point: the fake bevel
      // this replaced shaded all four edges of every cell and drew a grid
      // across a continuous surface.
      //
      // A static darken only, view-independent (no Fresnel/sheen term), so
      // it reads the same at any camera angle, and a narrow width with a
      // hard-ish falloff (pow, not a wide smoothstep) keeps it a crisp seam
      // rather than a haze. Width is in world units, so at distance it
      // converges to a faint darkening instead of turning a zoomed-out view
      // into a wireframe the way a screen-space outline would.
      // Interleaved gradient noise (Jimenez): one fract-heavy line, no
      // texture, and a far less regular pattern than an ordered Bayer matrix
      // at the same cost. Screen-space on purpose — that is what makes a
      // cross-fade read as a fade rather than as a texture stuck to the
      // surface. The price is that without temporal AA to average it, the
      // pattern is fixed to the screen while the geometry slides under it,
      // so a moving camera makes it crawl.
      float dither_threshold() {
        return fract(52.9829189 * fract(dot(gl_FragCoord.xy, vec2(0.06711056, 0.00583715))));
      }
      bool face_bit(float bit_value) {
        return mod(floor(floor(v_faces + 0.5) / bit_value), 2.0) >= 0.5;
      }
      // FACE_P/N per axis: 1,2 = +-X   4,8 = +-Y   16,32 = +-Z
      // RISES/DIPS per lateral: 64.. = rises   1024.. = dips
      // DIAG: 16384 = ++   32768 = +-   65536 = -+   131072 = --
      // CREASE per lateral: 262144.. — a lateral edge is convex by this and
      // not by its face bit, because a chunk-edge face is emitted without
      // its neighbour ever having been looked at.
      // CAP: 4194304 = above   8388608 = below. Also not the face bit: a
      // placed object is exempt from culling but is still solid, so a wall
      // capped by a user cube keeps its +Z face and is not a silhouette.
      float lateral_bit(bool x_axis, bool positive, float base) {
        return base * (x_axis ? (positive ? 1.0 : 2.0) : (positive ? 4.0 : 8.0));
      }
      float crease_dist(float d, bool convex, bool concave) {
        return (convex || concave) ? d : 1e9;
      }
      float edge_factor() {
        if (u_edge_strength <= 0.0) return 0.0;
        bool is_side = v_is_side > 0.5;
        vec3 dist_to_edge = (0.5 - abs(v_local_pos)) * v_box_scale;
        bool px = v_local_pos.x > 0.0, py = v_local_pos.y > 0.0, pz = v_local_pos.z > 0.0;
        bool x_dom = v_side_x_dominant > 0.5;
        bool n_positive = v_side_sign > 0.0;
        float d = 1e9;

        if (!is_side) {
          // Top or bottom face: both in-plane axes are lateral, and (d + n)
          // is that lateral neighbour one cell up (top) or down (bottom).
          float rise_or_dip = pz ? 64.0 : 1024.0;
          d = min(d, crease_dist(dist_to_edge.x, face_bit(lateral_bit(true, px, 262144.0)), face_bit(lateral_bit(true, px, rise_or_dip))));
          d = min(d, crease_dist(dist_to_edge.y, face_bit(lateral_bit(false, py, 262144.0)), face_bit(lateral_bit(false, py, rise_or_dip))));
        } else {
          // Side face. Its vertical edges are the wall's own top and bottom,
          // where (d + n) is this face's lateral neighbour one cell up or
          // down — the same bits the top face uses, read from the other
          // side. Its horizontal edge is the vertical seam, where (d + n) is
          // a diagonal in the XY plane.
          float own = lateral_bit(x_dom, n_positive, 1.0);
          d = min(d, crease_dist(dist_to_edge.z, !face_bit(pz ? 4194304.0 : 8388608.0), face_bit(own * (pz ? 64.0 : 1024.0))));
          bool sx = x_dom ? n_positive : px;
          bool sy = x_dom ? py : n_positive;
          float diag = sx ? (sy ? 16384.0 : 32768.0) : (sy ? 65536.0 : 131072.0);
          float horiz = x_dom ? dist_to_edge.y : dist_to_edge.x;
          bool horiz_convex = x_dom ? face_bit(lateral_bit(false, py, 262144.0)) : face_bit(lateral_bit(true, px, 262144.0));
          d = min(d, crease_dist(horiz, horiz_convex, face_bit(diag)));
        }
        return pow(1.0 - smoothstep(0.0, u_edge_width, d), 2.0);
      }
      vec3 apply_fake_bevel(vec3 color) {
        return color * mix(1.0, 1.0 - u_edge_strength, edge_factor());
      }`,
    )
    .replace(
      "void main() {",
      `void main() {
        bool v_inverted = v_visible > 1.5;
        float v_amount = v_inverted ? v_visible - 2.0 : v_visible;
        float v_pattern = dither_threshold();
        if (v_inverted ? v_pattern < v_amount : v_pattern >= v_amount) discard;`,
    )
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
  geo.setAttribute(FACES_ATTR, new THREE.BufferAttribute(built.faces, 1));
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
    push_edge_uniforms(shader);
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
type ShaderUniforms = { uniforms: Record<string, THREE.IUniform> };

function push_edge_uniforms(shader: ShaderUniforms): void {
  const debug = useDebugStore.getState();
  shader.uniforms.u_edge_strength.value = debug.edgeShadeStrength;
  shader.uniforms.u_edge_width.value = debug.edgeShadeWidth;
}

let shared_material: THREE.MeshStandardMaterial | null = null;

/**
 * Console handle for the two continuous knobs, in dev.
 *
 * The binary judgements — is edge shading on at all, does it touch vertical
 * faces — are toggles in the debug section, because they are the ones with
 * an obvious right answer once seen. Strength and width are not: they were
 * tuned when every cube edge was shaded and there is no reason the same
 * numbers read correctly now that a small fraction of edges are. Typing
 * `__pxl_edge({ strength: 0.35 })` and looking is faster than any slider.
 */
type EdgeGlobals = typeof globalThis & { __pxl_edge?: (next: { strength?: number; width?: number }) => void };

export function chunk_material(): THREE.MeshStandardMaterial {
  if (shared_material) return shared_material;
  shared_material = create_material();
  useSettingsStore.subscribe((state) => {
    const shader = shared_material?.userData.shader as ShaderUniforms | undefined;
    if (!shader) return;
    shader.uniforms.u_low_spec.value = state.low_spec;
    shader.uniforms.u_rim_intensity.value = state.show_rim_light ? rim_cfg.INTENSITY : 0;
    invalidate();
  });
  if (import.meta.env.DEV) {
    (globalThis as EdgeGlobals).__pxl_edge = ({ strength, width }) =>
      useDebugStore.getState().setEdgeShade({
        ...(strength === undefined ? {} : { edgeShadeStrength: strength }),
        ...(width === undefined ? {} : { edgeShadeWidth: width }),
      });
  }
  useDebugStore.subscribe(() => {
    const shader = shared_material?.userData.shader as ShaderUniforms | undefined;
    if (!shader) return;
    push_edge_uniforms(shader);
    invalidate();
  });
  return shared_material;
}

// @pxl/web/render/terrain/shader.ts

import { GROUND_CH_KEYS, GroundChannelKey } from "@pxl/shared/bake/format.ts";
import * as meta from "./shader_meta.ts";
import { shadow_cfg } from "@/modules/world/render/config.ts";

/** Splatmap sampler declarations — identical text needed in both create_vert
 * (low-spec's own resolve pass) and create_frag (texture-pack's, and
 * low-spec still needs ch_structure/etc. locals for the material-profile
 * effects further down regardless of mode). */
const ch_uniforms_decl = GROUND_CH_KEYS.map((ch_key) => `uniform sampler2D ${meta.gen.U_CH(ch_key)};`).join("\n  ");

/** `float ch_X = ...;` locals from the splatmap textures — same text needed
 * in both shader stages, just sampled at a different UV varying/local by
 * each (core_uv in the vertex shader where it's still a local; v_uv, the
 * same value passed through as a varying, in the fragment shader). */
const build_ch_sampling_logic = (uv_expr: string): string =>
  GROUND_CH_KEYS
    .map((ch_key) => `float ${meta.gen.CH(ch_key)} = floor(texture(${meta.gen.U_CH(ch_key)}, ${uv_expr}).r * 255.0 + 0.5);`)
    .join("\n    ");

/** Shared ground-layer resolver (see shader_meta.ts's own doc comment) —
 * generated once and included verbatim in both create_vert and create_frag,
 * since each is its own compiled GLSL program with no cross-shader sharing;
 * textually identical either way, so the two stages can never disagree
 * about what a given channel/type combination resolves to. */
const ground_layer_resolver = meta.build_ground_layer_resolver(meta.get_layer_meta());

/**
 * Resolves one ground-layer color at an arbitrary channel-sample UV —
 * `sample_uv` picks which category wins (splatmap channels + resolve_
 * ground_layer's priority), `box_uv` is the tiling coordinate across the
 * cell's own face (unrelated to which fine sub-cell is being categorized),
 * `is_side` picks u_ground_layers_side over u_ground_layers (mat.ts builds
 * both from the same layer indexing — a type with no distinct side texture
 * just resolves to the same image in both, see ground_layer_images.ts's
 * GROUND_LAYER_SIDE_IMAGE_MAP). Fragment-only (u_ground_layers/
 * resolve_ground_layer aren't declared in create_vert's low-spec path) —
 * factored out so create_frag's main() can call it at several different
 * sample_uv taps for the coarse-LOD supersample blend below, instead of
 * repeating the channel-sample+resolve+fetch sequence at each tap site.
 */
const ground_layer_color_sampler = /* glsl */ `
  vec3 sample_ground_layer_color(vec2 sample_uv, vec2 box_uv, bool is_side) {
    ${build_ch_sampling_logic("sample_uv")}
    float ground_layer = resolve_ground_layer(ch_land, ch_water, ch_structure, ch_transport, u_show_structures);
    return is_side
      ? texture(u_ground_layers_side, vec3(box_uv, ground_layer)).rgb
      : texture(u_ground_layers, vec3(box_uv, ground_layer)).rgb;
  }
`;

/**
 * Shared position-computation core.
 *
 * Every cell's final object-space position (LOD-quantized height sampling,
 * instance placement, skirt drop) is identical for the normal color pass
 * and the shadow-depth pass casting this terrain's own elevated cells (no
 * separate 3D building mesh yet — a "building" here just *is* a tall
 * terrain cell, see chunk.tsx/mat.ts). Sharing this between `vert` and
 * `depth_vert` keeps a shadow's silhouette from ever drifting off the
 * cell's actual shape, instead of hand-maintaining the same math twice.
 *
 * Writes local vars only, not varyings (core_uv/core_is_skirt/
 * core_is_side_face/transformed), so each caller picks what it needs — the
 * depth pass only needs transformed + core_uv for the edit-mask discard.
 */
const position_core = /* glsl */ `
    int i_lod_res = int(u_row_stride / u_lod_step);

    int ix_lod = gl_InstanceID % i_lod_res;
    int iy_lod = gl_InstanceID / i_lod_res;

    // ---- 0. LOD-boundary blend: which of this pair's two draws claims this chunk ----
    //
    // u_blend_role/u_blend_t are only meaningful while this chunk is inside a
    // transition band (get_lod_guide's blend_active in lod.ts) — outside one,
    // chunk.tsx leaves role at 0 (finer) and t at 0, which makes core_visible
    // unconditionally true below, i.e. exactly today's behavior with one
    // extra always-true comparison.
    //
    // u_blend_r is a per-CHUNK constant (computed once in JS from page_idx+
    // page_off — see chunk.tsx), not a per-cell hash. Used to be the latter
    // (each cell independently coin-flipped between this draw and its
    // blend-partner draw, via a hash of the cell's own (ix,iy)) — that
    // partitioned color smoothly (mip levels are already a soft per-cell
    // difference) but partitioned HEIGHT into a visible checkerboard: a
    // coarse-LOD cell's box-filtered height and its fine-LOD neighbor's raw
    // height can differ by a real amount, and interleaving the two per-cell
    // reads as relief noise, not a gradient, the way the same trick reads
    // fine for color. Since both draws of one chunk share this same u_blend_r
    // value, this chunk-uniform version makes the visibility split atomic
    // per chunk instead — its whole population of cells hard-pops together,
    // eliminating the intra-chunk checkerboard entirely. What blending
    // originally existed for (avoiding a hard seam between two ADJACENT
    // chunks that land on opposite sides of the same threshold at the same
    // instant) is preserved: each chunk still has its own distinct u_blend_r,
    // so different chunks still cross u_blend_t at different points within
    // the shared band, staggering the population-level transition instead of
    // every loaded chunk switching in the same frame.
    bool core_visible = (u_blend_role < 0.5) ? (u_blend_r >= u_blend_t) : (u_blend_r < u_blend_t);

    // ---- 1. index and sampling coordinate (tex_coord) ----

    int ix = ix_lod * int(u_lod_step);
    int iy = iy_lod * int(u_lod_step);
    vec2 tex_coord = vec2(float(ix), float(iy));

    vec2 core_uv = (tex_coord + 0.5) / u_row_stride;

    // ---- 2. elevation sampling (surface_z) ----

    /**
     * A coarse-LOD cell covers an NxN block of fine cells (N = u_lod_step),
     * but used to always read just one texel (the block's corner) — plain
     * nearest-neighbor downsampling, and a real aliasing source distinct
     * from LOD-boundary popping: that one texel could be an outlier (a
     * curb, a doorway) unrepresentative of the whole coarse cell.
     *
     * Hardware mipmaps can't fix it — u_heightmap is an isampler2D
     * (texelFetch's exact integer access is required so height stays a real
     * elevation, not a filtered fraction), and integer textures can't use a
     * filtering sampler on any graphics API. Done by hand instead: a fixed
     * 4-tap sparse box filter (1/4 and 3/4 points, rotated-grid) averaged
     * into one height. Fixed at 4 taps regardless of block size (sampling
     * all 1024 texels of a 32x32 block per vertex isn't worth it) — cheap
     * enough to always run, and coarse tiers have far fewer live instances
     * anyway (instance_cnt shrinks with lod_step², see lod.ts), so the extra
     * taps land where the instance count is smallest.
     */
    int h;
    if (u_lod_step <= 1.0) {
      h = texelFetch(u_heightmap, ivec2(ix, iy), 0).r;
    } else {
      int step_i = int(u_lod_step);
      int row_max = int(u_row_stride) - 1;
      int q1 = step_i / 4;
      int q3 = (step_i * 3) / 4;
      int h00 = texelFetch(u_heightmap, ivec2(min(ix + q1, row_max), min(iy + q1, row_max)), 0).r;
      int h01 = texelFetch(u_heightmap, ivec2(min(ix + q1, row_max), min(iy + q3, row_max)), 0).r;
      int h10 = texelFetch(u_heightmap, ivec2(min(ix + q3, row_max), min(iy + q1, row_max)), 0).r;
      int h11 = texelFetch(u_heightmap, ivec2(min(ix + q3, row_max), min(iy + q3, row_max)), 0).r;
      // round to nearest, not GLSL's truncating int division — see the
      // Z-quantization line below for why a downward bias here specifically
      // (as opposed to just some fixed sampling error) reads as visible
      // popping during panning.
      h = int(round((float(h00) + float(h01) + float(h10) + float(h11)) * 0.25));
    }
    // Z-quantization: snap to a multiple of the LOD step — needed because
    // visual_scale (below) scales X, Y, *and* Z together by u_lod_step, so a
    // coarse cell reads as one enlarged voxel only if its height is also a
    // whole multiple of that same enlarged unit; an unsnapped height would
    // sit at a fraction of a coarse voxel, breaking the blocky look.
    //
    // floor(), deliberately not round(): round() picks whichever of the two
    // neighboring step-multiples is numerically closer, independent of
    // step size — for a fixed true height that can pick the *lower*
    // multiple at one step and the *higher* one at a coarser step (e.g.
    // true height 1: step 4 rounds to 0, step 2 rounds to 2, step 1 is
    // exactly 1 — 0 -> 2 -> 1 isn't a monotonic falloff, it's a bounce, and
    // reads as the cell popping *up* momentarily while zooming through a
    // coarser tier before dropping back down). floor(h/step)*step is
    // provably monotonic non-increasing in step for any fixed h (the
    // largest step-multiple <= h can only shrink or hold as step grows,
    // never grow) — every coarser tier is <= the next finer one, always,
    // for every cell, so panning/zooming through the LOD ladder only ever
    // sees a cell's height count down toward its true value, never bounce.
    // The 4-tap average above stays round()-based regardless — that step
    // has no "which grid tier" comparison to stay consistent across, it's
    // just estimating one block's mean height, where round() is the
    // unbiased choice.
    h = int(floor(float(h) / u_lod_step)) * int(u_lod_step);

    // 2. physical height calculation and expansion
    float surface_z = float(h) * u_scale; // real-world elevation (top face)
    float visual_scale = u_scale * u_lod_step; // size of one voxel cell (X/Y/Z scaled together)

    // ---- 3. instance position (instance_pos) ----

    // 3. Z-axis shift (position offset)
    float offset = float(i_lod_res) * 0.5;
    vec3 instance_pos = vec3(
      (float(ix_lod) - offset + 0.5) * visual_scale,
      (float(iy_lod) - offset + 0.5) * visual_scale,
      surface_z - (visual_scale * 0.5) // this is the actual Z shift
    );

    vec3 local_pos = position;

    // ---- 4. neighbor sampling index setup ----

    int i_res = int(u_row_stride);
    int i_lod = int(u_lod_step);
    bool is_edge_x = (ix_lod == 0 || ix_lod == i_lod_res - 1);
    bool is_edge_y = (iy_lod == 0 || iy_lod == i_lod_res - 1);

    int iy_n = clamp(iy + i_lod, 0, i_res - 1);
    int iy_s = clamp(iy - i_lod, 0, i_res - 1);
    int ix_e = clamp(ix + i_lod, 0, i_res - 1);
    int ix_w = clamp(ix - i_lod, 0, i_res - 1);

    float h_n = float(texelFetch(u_heightmap, ivec2(ix, iy_n), 0).r) * u_scale;
    float h_s = float(texelFetch(u_heightmap, ivec2(ix, iy_s), 0).r) * u_scale;
    float h_e = float(texelFetch(u_heightmap, ivec2(ix_e, iy), 0).r) * u_scale;
    float h_w = float(texelFetch(u_heightmap, ivec2(ix_w, iy), 0).r) * u_scale;

    // A cheap neighbor-height AO term (darken a cell tucked against taller
    // neighbors) used to live here, computed from these same h_n/h_s/h_e/h_w
    // samples. Removed: it was the actual source of the pervasive
    // crosshatch/speckle-at-distance bug (confirmed by bypassing it — the
    // pattern vanished completely) and had no principled reason to coexist
    // with N8AO's real screen-space contact AO (scene.tsx) once that was
    // working — two independent, uncoordinated darkening terms stacking on
    // top of each other rather than one correct one. See git history for
    // the removed implementation if this needs revisiting.

    // ---- 5. skirt bottom-edge calculation ----

    // interior neighbor heights are raw (non-quantized) samples, so near a steep
    // discontinuity (coastline, cliff) a neighbor can read far lower than this
    // cell's own height — without a bound, extra_drop below stretches the cell
    // into an arbitrarily long slab. Cap it to a few cell-heights: enough to hide
    // ordinary LOD-quantization gaps between neighbors, not real terrain drops
    // (those should just show as a natural vertical face, not a giant skirt).
    const float MAX_INTERIOR_SKIRT_CELLS = 4.0;

    float cell_bottom_z = surface_z - visual_scale;

    float target_bottom;
    if (is_edge_x || is_edge_y) {
      target_bottom = surface_z - 200.0;
    } else {
      float neighbor_min = min(min(h_n, h_s), min(h_e, h_w)) - visual_scale;
      target_bottom = max(neighbor_min, cell_bottom_z - MAX_INTERIOR_SKIRT_CELLS * visual_scale);
    }

    float core_is_skirt = 0.0;

    if (local_pos.z < 0.0 && target_bottom < cell_bottom_z) {
      float extra_drop = (cell_bottom_z - target_bottom) / visual_scale;
      local_pos.z -= extra_drop;
      if (is_edge_x || is_edge_y) core_is_skirt = 1.0;
    }

    // CHUNK_BOX's built-in 'normal' attribute is flat per face (BoxGeometry
    // gives each face its own hard-edged normal, never smoothed across an
    // edge) — every vertex belonging to one wall face agrees normal.z ≈ 0,
    // every vertex of the top/bottom faces agrees normal.z ≈ ±1, with no
    // exceptions. That flatness is exactly why this needs to key off normal
    // instead of position.z: a wall face's four corners span the *entire*
    // z range (two at +0.5, two at -0.5, since it's a full-height wall), so
    // a 'position.z < threshold' test disagreed between a wall's own top-
    // edge vertices (false) and bottom-edge vertices (true) — and since
    // this value feeds a smoothly-interpolated varying (v_is_side_face,
    // read by both the vertex-side texture selection AND every side_diff/
    // skirt tint below), the disagreement didn't just misclassify a few
    // vertices, it produced a real 0->1 gradient down every wall's own
    // height: the bottom half correctly read as "side" while the top half
    // silently fell back to the top texture/tint, mid-face, on every
    // single wall — worse up close (the gradient band fills the screen)
    // than from a distance (it minifies into a blend that merely looks
    // "kind of side-colored"), which is exactly the "fine far away, a
    // mirage up close" symptom this was reported as.
    float core_is_side_face = (abs(normal.z) < 0.5) ? 1.0 : 0.0;

    vec3 scaled_local_pos = local_pos * visual_scale * 1.002;
    vec3 transformed = scaled_local_pos + instance_pos;
`;

// vertex shader
export function create_vert(): string {
  return /* glsl */ `
  precision highp float;
  precision highp int;
  precision highp isampler2D;
  #include <common>
  #include <fog_pars_vertex>
  #include <shadowmap_pars_vertex>

  out vec2 v_uv;
  out vec2 v_box_uv;
  out float v_is_side_face;
  out float v_is_skirt;
  out float v_view_dist;
  out vec3 v_view_pos;
  out float v_surface_z;
  out vec2 v_world_xy;
  // raw, pre-skirt box-local vertex position (position attribute, [-0.5,
  // 0.5] per axis) — not the same as position_core's local scratch var of
  // the same name, which the skirt logic stretches on the Z axis. The
  // fragment shader uses the untouched version to fake a bevel (see its own
  // comment) — a skirt is invisible fill geometry hidden under the terrain,
  // not something that needs a chamfered edge, so bending its normal based
  // on the *stretched* position would just be wrong.
  out vec3 v_local_pos;
  out vec3 v_ground_color;
  // pre-division tex_coord (see position_core's step 1) — passed through
  // rather than reconstructed from v_uv in the fragment shader so the
  // coarse-LOD supersample taps there (create_frag's u_low_spec-false
  // branch) stay exact integer-grid offsets instead of round-tripping
  // through a UV division/multiplication. flat: this is a per-instance
  // constant (same reasoning as core_uv itself), not something meant to
  // interpolate.
  flat out vec2 v_tex_coord;

  uniform isampler2D u_heightmap;
  uniform float u_scale;
  uniform float u_row_stride;
  uniform float u_lod_step;
  // see position_core's step 0 above — chunk.tsx only sets these away from
  // their (0, 0) defaults on the two meshes it mounts while inside a
  // get_lod_guide blend band.
  uniform float u_blend_t;
  uniform float u_blend_role;
  // per-chunk constant (see position_core's step 0) — same value on both of
  // this chunk's materials, set once at chunk creation, never touched per-frame.
  uniform float u_blend_r;

  uniform bool u_low_spec;

  // real per-chunk mip-blended color bake (shader_meta.ts's
  // decode_ground_rgba + tex.ts's create_ground_color_texture) — low-spec
  // mode's own color source; unlike u_ground_layers (create_frag's
  // texture-pack path), this is real per-cell color baked once per chunk,
  // so hardware mip generation gives real cross-category blending at
  // coarse LOD instead of resolving to one winning category per sample.
  uniform sampler2D u_ground_color;

  void main() {
    ${position_core}

    v_uv = core_uv;
    v_box_uv = uv;
    v_tex_coord = tex_coord;
    v_is_skirt = core_is_skirt;
    v_is_side_face = core_is_side_face;
    v_local_pos = position;

    // ---- ground color: low-spec path only (create_frag's own u_low_spec
    // branch handles texture-pack mode instead, from u_ground_layers) —
    // real mip-sampled fetch, not a nearest corner texel. u_lod_step is
    // always a power of two in {1,2,4,8,16,32}, so log2() lands exactly on
    // an integer mip level (0-5) — textureLod, not the derivative-based
    // texture(), because core_uv is constant across one instance's own
    // face (position-computed per-instance, not per-vertex), so its
    // screen-space derivative is ~0 and implicit LOD selection would always
    // resolve to mip 0 on its own.
    //
    // NOT core_uv: that's centered on this block's first *fine* sub-cell
    // (tex_coord = ix_lod*lod_step, offset toward one corner of the block —
    // fine for a nearest fetch of one texel, which is all it was ever built
    // for), not on the block's own center in mip-space. Sampling a mip level
    // at an off-center UV pulls a large, systematically-biased fraction of
    // its bilinear weight from the *neighboring* block instead of this one —
    // at high LOD steps that's most of the sample, which read as a
    // pervasive blur/haze across every coarse-LOD cell once this shipped.
    // ground_uv instead centers on (ix_lod, iy_lod) itself — the same
    // block-index-plus-half-cell centering position_core's own instance
    // placement already uses (see instance_pos below) — which collapses to
    // exactly core_uv when lod_step is 1 (ix_lod == ix), so the finest LOD
    // tier is unaffected.
    if (u_low_spec) {
      vec2 ground_uv = (vec2(ix_lod, iy_lod) + 0.5) * u_lod_step / u_row_stride;
      v_ground_color = textureLod(u_ground_color, ground_uv, log2(u_lod_step)).rgb;
    } else {
      // unused by create_frag in this mode — see its own u_low_spec branch.
      v_ground_color = vec3(0.0);
    }

    // ---- 6. Shadow-map coordinates ----
    //
    // three's <shadowmap_vertex> wants a real per-vertex normal (it offsets
    // the sampled position along it — shadowNormalBias — to fight acne). This
    // terrain doesn't carry one: the fragment shader derives its shading
    // normal from screen-space derivatives of view position instead (cheaper,
    // and correct for the faceted box-per-cell look). A fixed up-vector is a
    // fine stand-in here specifically for the bias offset's purpose — most of
    // the terrain genuinely is close to horizontal, and getting this exact
    // for the (rarer) side faces would only nudge shadow-acne bias slightly,
    // not the shadow's position/shape.
    vec3 transformedNormal = vec3(0.0, 0.0, 1.0);
    #include <worldpos_vertex>
    #include <shadowmap_vertex>

    vec4 world_pos = modelMatrix * vec4(transformed, 1.0);
    vec4 mvPosition = viewMatrix * world_pos;

    gl_Position = projectionMatrix * mvPosition;
    // push it outside the clip volume rather than branching around the rest
    // of main() — cheap, and every varying below stays well-defined either way.
    if (!core_visible) gl_Position = vec4(2.0, 2.0, 2.0, 1.0);

    v_view_pos = mvPosition.xyz;
    v_view_dist = -mvPosition.z;
    v_surface_z = surface_z;
    v_world_xy = world_pos.xy;

    #include <fog_vertex>
  }
  `;
}

/**
 * Shadow-depth pass.
 *
 * Assigned as this terrain's customDepthMaterial (chunk.tsx) so tall cells
 * actually occlude light — without it, castShadow would fall back to
 * three's default depth material, which doesn't know this geometry
 * displaces per-instance by the heightmap (every cell would render flat and
 * undisplaced — same failure mode as the N8AO postmortem in scene.tsx).
 * Reuses position_core so the caster's silhouette matches what's actually
 * drawn, then does the minimum a depth-only pass needs: no lighting, no
 * fog, just gl_Position (the rasterizer's own depth write is what matters
 * for PCF/PCFSoft; the fragment shader only exists to satisfy GLSL3's
 * main() requirement and to discard edited/removed cells).
 */
export const depth_vert = /* glsl */ `
  precision highp float;
  precision highp int;
  precision highp isampler2D;

  out vec2 v_uv;

  uniform isampler2D u_heightmap;
  uniform float u_scale;
  uniform float u_row_stride;
  uniform float u_lod_step;
  uniform float u_blend_t;
  uniform float u_blend_role;
  uniform float u_blend_r;

  void main() {
    ${position_core}

    v_uv = core_uv;
    gl_Position = projectionMatrix * viewMatrix * modelMatrix * vec4(transformed, 1.0);
    if (!core_visible) gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
  }
`;

export const depth_frag = /* glsl */ `
  precision highp float;

  in vec2 v_uv;
  uniform sampler2D u_edit_mask;

  out vec4 out_color;

  void main() {
    if (texture(u_edit_mask, v_uv).r > 0.5) discard;
    out_color = vec4(1.0);
  }
`;

// fragment shader
export function create_frag(): string {
  return /* glsl */ `
  precision highp float;
  #include <common>
  #include <fog_pars_fragment>
  // bound automatically per-object by WebGLRenderer regardless of material
  // type (see renderer.js's setProgram — it's a plain reflected-uniform
  // write, not gated on material.isMeshStandardMaterial etc.), same as
  // <shadowmap_pars_fragment>'s own uniforms below; this one just isn't
  // declared by any chunk we're pulling in on its own, since we skip the
  // (much larger, PBR-lights-focused) <lights_pars_begin> that normally
  // carries it.
  uniform bool receiveShadow;
  #include <shadowmap_pars_fragment>
  #include <shadowmask_pars_fragment>

  ${ch_uniforms_decl}
  uniform sampler2D u_edit_mask;

  uniform float u_border_opacity;
  uniform vec3 u_light_dir;
  // debug toggle (dev_panel.tsx's "건물" button, off by default) — see
  // is_structure below, the single point every structure-specific fragment
  // effect (blending, material grain, side-face tint, ...) already keys off,
  // so gating it there is enough to make hidden structure cells fall all the
  // way back through to whatever's underneath (land/water/transport) rather
  // than needing a separate check at each downstream usage.
  uniform bool u_show_structures;

  // see create_vert's own copy of these three — u_low_spec picks which of
  // the two shaders actually resolves+samples the ground color this frame
  // (vertex there, fragment here); u_ground_layers/resolve_ground_layer are
  // the same shared texture/function either way (see shader_meta.ts).
  uniform sampler2DArray u_ground_layers;
  uniform sampler2DArray u_ground_layers_side;
  uniform bool u_low_spec;
  // needed only to regenerate the coarse-LOD supersample taps below (see
  // v_tex_coord's own comment in create_vert) — u_low_spec's low-spec branch
  // doesn't touch these, only the texture-pack else-branch does.
  uniform float u_row_stride;
  uniform float u_lod_step;

  in vec2 v_uv;
  in vec2 v_box_uv;
  flat in vec2 v_tex_coord;
  in float v_is_side_face;
  in float v_is_skirt;
  in float v_view_dist;
  in vec3 v_view_pos;
  in float v_surface_z;
  in vec2 v_world_xy;
  in vec3 v_local_pos;
  in vec3 v_ground_color;

  out vec4 out_color;

  ${ground_layer_resolver}
  ${ground_layer_color_sampler}

  float getDynamicGrid(vec2 uv, float target_width) {
    vec2 fw = fwidth(uv);
    vec2 dist = abs(fract(uv - 0.5) - 0.5);
    vec2 norm_dist = dist / fw;
    float line_x = 1.0 - smoothstep(target_width - 0.5, target_width + 0.5, norm_dist.x);
    float line_y = 1.0 - smoothstep(target_width - 0.5, target_width + 0.5, norm_dist.y);
    return max(line_x, line_y);
  }

  float get_luminance(vec3 color) {
    return dot(color, vec3(0.299, 0.587, 0.114));
  }

  // saturate() comes from <common> above (a #define, not a function — GLSL's
  // preprocessor would have mangled a same-named function decl placed after
  // it, so this used to shadow that macro instead of reusing it).

  vec3 rotate_tint(vec3 base, vec3 tint, float strength) {
    return mix(base, base * tint, strength);
  }

  float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
  }

  void main() {
    float edit_mask = texture(u_edit_mask, v_uv).r;
    if (edit_mask > 0.5) discard;

    ${build_ch_sampling_logic("v_uv")}
    bool is_water = ${meta.gen.CH(GroundChannelKey.WATER)} > 0.5;
    bool is_land = ${meta.gen.CH(GroundChannelKey.LAND)} > 0.5;
    bool is_structure = ${meta.gen.CH(GroundChannelKey.STRUCTURE)} > 0.5 && u_show_structures;
    bool is_transport = ${meta.gen.CH(GroundChannelKey.TRANSPORT)} > 0.5;

    // low-spec: v_ground_color was already resolved+sampled once per vertex
    // (create_vert) at a fixed UV — cheap, flat per cell. texture-pack: same
    // resolve_ground_layer priority (LAND -> WATER -> STRUCTURE(if shown) ->
    // TRANSPORT), sampled here per-fragment with a real tiling UV (v_box_uv)
    // so a real (non-flat-fill) layer image actually shows detail across a
    // cell's face — see shader_meta.ts's build_ground_layer_resolver for why
    // this priority logic is generated once and shared verbatim with
    // create_vert instead of living in each place separately.
    // top vs side texture selection (u_ground_layers vs u_ground_layers_side)
    // — see sample_ground_layer_color's own comment. Only meaningful in
    // texture-pack mode; low-spec's v_ground_color never distinguishes faces
    // (it never did, and this isn't the ask that changed).
    bool is_side = v_is_side_face > 0.5;

    vec3 final_color;
    if (u_low_spec) {
      final_color = v_ground_color;
    } else if (u_lod_step <= 1.0) {
      // finest tier: one macro-cell IS one fine cell, so supersampling
      // within it would just resample the same one splatmap texel four
      // times over — skip straight to the single-tap read.
      final_color = sample_ground_layer_color(v_uv, v_box_uv, is_side);
    } else {
      // Coarse-LOD supersample blend — the category-resolve analogue of
      // position_core's own 4-tap sparse box filter for height (same
      // rotated-grid 1/4/3/4 tap positions, see that comment for why this
      // pattern and tap count). A macro-cell here spans u_lod_step fine
      // cells, but v_uv/ch_* only ever sampled ONE corner texel of that
      // whole block (a real aliasing source — a road one fine-cell wide can
      // fall entirely outside that one sample point and vanish completely,
      // then reappear a frame later as the LOD grid shifts, reading as an
      // abrupt pop rather than a graceful fade). Category ids can't be
      // hardware-mipmapped (averaging two ids isn't a third valid id, same
      // reason splatmaps are NearestFilter/no-mips in tex.ts), so this
      // hand-rolled supersample stands in for it: resolve+sample each tap's
      // own winning color independently, then average the *colors* — a
      // thin/rare category's contribution now shrinks smoothly toward zero
      // as distance grows instead of blinking in and out.
      float q1 = u_lod_step * 0.25;
      float q3 = u_lod_step * 0.75;
      vec3 c00 = sample_ground_layer_color((v_tex_coord + vec2(q1, q1) + 0.5) / u_row_stride, v_box_uv, is_side);
      vec3 c01 = sample_ground_layer_color((v_tex_coord + vec2(q1, q3) + 0.5) / u_row_stride, v_box_uv, is_side);
      vec3 c10 = sample_ground_layer_color((v_tex_coord + vec2(q3, q1) + 0.5) / u_row_stride, v_box_uv, is_side);
      vec3 c11 = sample_ground_layer_color((v_tex_coord + vec2(q3, q3) + 0.5) / u_row_stride, v_box_uv, is_side);
      final_color = (c00 + c01 + c10 + c11) * 0.25;
    }

    vec3 fdx = dFdx(v_view_pos);
    vec3 fdy = dFdy(v_view_pos);
    vec3 normal = normalize(cross(fdx, fdy));

    // ---- fake bevel: bend the flat per-face normal near box edges/corners ----
    // CHUNK_BOX is a plain unit cube with no real chamfer, so the normal is
    // flat right up to a hard edge. Classic "rounded box" SDF-gradient trick
    // instead of modeling an actual chamfer (16384 instances/chunk — real
    // bevel geometry would cost real vertices for what this gets for free):
    // near an edge/corner, bend the normal toward the diagonal a real
    // chamfer would have. Every lighting term below (diff, side_diff, water
    // fresnel/specular) just reads whatever normal holds, so this one change
    // gives all of them a believable highlight/shadow where a physical bevel
    // would catch or lose light — not a separate brightness hack.
    // skipped on skirts: invisible fill geometry, not a real edge to chamfer.
    if (v_is_skirt < 0.5) {
      const float BEVEL_WIDTH = 0.14;
      vec3 corner = sign(v_local_pos) * max(abs(v_local_pos) - (0.5 - BEVEL_WIDTH), 0.0);
      float corner_len = length(corner);
      if (corner_len > 0.0001) {
        // v_local_pos is object-space, normal is view-space (derived from
        // v_view_pos) — same reason light_dir gets run through viewMatrix
        // just below instead of dotting across two different frames.
        vec3 corner_view = normalize((viewMatrix * vec4(corner, 0.0)).xyz);
        normal = normalize(mix(normal, corner_view, smoothstep(0.0, BEVEL_WIDTH * 1.4, corner_len)));
      }
    }

    vec3 view_dir = normalize(-v_view_pos);

    // u_light_dir tracks the real sun (see sun_dir.ts) in world space; normal
    // above is derived from v_view_pos, i.e. view space — bring the light
    // direction into the same space rather than dotting across two frames.
    vec3 light_dir = normalize((viewMatrix * vec4(u_light_dir, 0.0)).xyz);
    float diff = max(dot(normal, light_dir), 0.0);

    // shared fade for every *procedural* per-cell detail effect below (side
    // tint/darkening, skirt darkening, material grain, height tone, water
    // shimmer) — anything that varies per-fragment from a hash/sin with no
    // mip chain to filter against, and so aliases into a moiré/speckle once
    // enough cells minify into a few screen pixels (a per-cell neighbor-height
    // AO term used to live here too, until it turned out to *be* that exact
    // bug — see position_core's doc comment on why it was removed instead of
    // just faded). One shared value instead of each effect rolling its own
    // copy of the same smoothstep — that duplication is exactly how h_tone
    // and the water shimmer effect further down ended up missed for as long
    // as they did; a new per-cell effect added here now has to deliberately
    // opt OUT of fading (not remember to opt in).
    // not used for the grid lines (their own fade also adjusts line width,
    // not just opacity — see that comment) or real shadow (shadow_cfg's
    // separate, origin-distance-based fade, immediately below).
    float detail_fade = 1.0 - smoothstep(60.0, 220.0, v_view_dist);

    // real cast shadows (buildings onto ground) via three's own shadow-map
    // uniforms — see shadowmap_pars_vertex/_fragment above and mat.ts's
    // lights:true. getShadowMask() returns 1.0 (fully lit) down to 0.0
    // (fully shadowed) per directional/spot/point light with castShadow; only
    // the sun/moon directional lights cast here (environment.tsx). Only the
    // sun's own diffuse contribution is attenuated, not the 0.82 ambient
    // floor — this material's whole look leans flat/toy-ambient on purpose,
    // so a shadow that crushed straight to black would read as a rendering
    // glitch, not a shadow. mix() keeps a visible but bounded dip instead.
    float shadow_mask = getShadowMask();

    // fade the shadow's influence to nothing before the shadow camera's
    // actual frustum edge (see shadow_cfg's doc comment) — otherwise PCF
    // sampling right at that hard boundary clamps to edge texels and reads
    // as a flat dark rectangle sitting in the middle of the view, which
    // looked far worse than just not having a shadow there.
    float shadow_dist = length(v_world_xy);
    float shadow_edge_fade = 1.0 - smoothstep(${shadow_cfg.FADE_START.toFixed(1)}, ${shadow_cfg.FADE_END.toFixed(1)}, shadow_dist);
    shadow_mask = mix(1.0, shadow_mask, shadow_edge_fade);

    // ambient-occlusion contact darkening is N8AO's job now (real
    // screen-space AO, see scene.tsx) — this material no longer rolls its
    // own per-cell approximation of the same thing (see position_core's doc
    // comment for why the old one was removed rather than kept alongside it).
    float light_intensity = 0.82 + (0.18 * diff * mix(0.4, 1.0, shadow_mask));
    final_color *= light_intensity;

    // ---- channel material profiles ----
    //
    // goal:
    // - land / grass: softer and warmer
    // - roads: flatter / cleaner
    // - structures: denser / more matte
    if (v_is_skirt < 0.5 && v_is_side_face < 0.5) {
      if (is_land && !is_water) {
        final_color *= vec3(1.008, 1.006, 1.000);
      }

      if (is_transport) {
        final_color *= vec3(0.998, 0.998, 1.002);
      }

      if (is_structure) {
        final_color *= vec3(0.992, 0.990, 0.988);
      }
    }

    // ---- subtle land height tone variation ----
    //
    // goal:
    // - make terrain feel less flat / printed
    // - keep it extremely soft, toy-like
    if (!is_water && v_is_skirt < 0.5 && v_is_side_face < 0.5) {
      // v_surface_z is real per-cell elevation, which is exactly why this
      // needs detail_fade too: it's smooth across open flat ground, but
      // jumps abruptly between adjacent cells right at a building's edge —
      // dense areas with lots of those edges turn h_tone's swing into the
      // same high-frequency, un-filterable pattern as the grain hash below.
      float h_tone = 0.5 + 0.5 * sin(v_surface_z * 0.035);
      vec3 hi_tint = vec3(1.015, 1.012, 1.000);
      vec3 lo_tint = vec3(0.992, 0.992, 0.998);
      final_color *= mix(vec3(1.0), mix(lo_tint, hi_tint, h_tone), detail_fade);

      // subtle material grain / roughness-like variation
      //
      // this hashes per ~1.4-2.9 world-unit cell (0.70/0.35 frequency) —
      // high enough frequency that once many of those cells minify into a
      // handful of screen pixels (any view beyond a couple hundred units),
      // it stops reading as "grain" and turns into a swimming moiré/crosshatch
      // instead — exactly the "looks cheap" pattern reported at distance. There's no
      // mip chain for a procedural per-fragment hash to filter against (unlike
      // a real texture), so the fix is fading the effect out with distance
      // rather than trying to band-limit it.
      float grain_a = hash21(floor(v_world_xy * 0.35));
      float grain_b = hash21(floor(v_world_xy * 0.70 + vec2(11.0, 7.0)));
      float grain = mix(grain_a, grain_b, 0.5);
      float rough_lo = 0.985;
      float rough_hi = 1.015;

      if (is_transport) {
        rough_lo = 0.994;
        rough_hi = 1.004;
      } else if (is_structure) {
        rough_lo = 0.989;
        rough_hi = 1.006;
      } else if (is_land) {
        rough_lo = 0.982;
        rough_hi = 1.018;
      }

      float rough_tone = mix(1.0, mix(rough_lo, rough_hi, grain), detail_fade);
      final_color *= vec3(rough_tone, rough_tone, rough_tone * 0.998);
    }

    // ---- directional side-face/skirt lighting ----
    // goal: break the uniform block-wall look, keep side faces warm and miniature-like
    //
    // used to be a fake compass-direction palette (east always warm, west
    // always cool, regardless of the real sun) — backwards from the top
    // face above, which already computes a real Lambertian term (diff) from
    // the real sun direction and normal. Side faces have that same normal
    // (screen-space derivatives, correct for either face) and never used
    // it. side_diff is that same real term evaluated here: a wall actually
    // facing the sun reads bright/warm, one facing away reads dark/cool,
    // correct regardless of terrain orientation — and since it's driven by
    // smooth real geometry+light rather than a per-fragment hash, it needs
    // no distance fade (unlike the genuinely fake effects above).
    float side_diff = max(dot(normal, light_dir), 0.0);

    if (v_is_side_face > 0.5 && v_is_skirt < 0.5) {
      vec3 lit_tint = vec3(1.02, 0.99, 0.95);
      vec3 shadow_tint = vec3(0.93, 0.94, 0.97);
      vec3 side_tint = mix(shadow_tint, lit_tint, side_diff);

      float side_strength = 0.18;
      if (is_transport) {
        side_strength = 0.10;
      } else if (is_structure) {
        side_strength = 0.14;
      } else if (is_land && !is_water) {
        side_strength = 0.22;
      }

      final_color = rotate_tint(final_color, side_tint, side_strength);
    }

    if (v_is_side_face > 0.5) {
      vec3 side_mul_lit = vec3(1.0, 0.98, 0.94);
      vec3 side_mul_shadow = vec3(0.83, 0.80, 0.76);

      if (is_transport) {
        side_mul_lit = vec3(1.0, 0.995, 0.99);
        side_mul_shadow = vec3(0.90, 0.89, 0.88);
      } else if (is_structure) {
        side_mul_lit = vec3(0.99, 0.98, 0.97);
        side_mul_shadow = vec3(0.86, 0.84, 0.82);
      }

      final_color *= mix(side_mul_shadow, side_mul_lit, side_diff);
    }

    if (v_is_skirt > 0.5) {
      vec3 skirt_lit = vec3(0.96, 0.94, 0.90);
      vec3 skirt_shadow = vec3(0.80, 0.77, 0.73);
      final_color *= mix(skirt_shadow, skirt_lit, side_diff);
    }

    if (v_is_skirt < 0.5 && v_is_side_face < 0.5) {
      final_color *= vec3(1.02, 1.01, 0.99);
    }

    // ---- water material lift ----
    //
    // goal:
    // - slightly cleaner / brighter than land
    // - less clay-like side tint
    // - softer top-face highlight
    if (is_water) {
      final_color *= vec3(0.99, 1.04, 1.07);

      if (v_is_side_face > 0.5) {
        final_color *= vec3(1.02, 1.04, 1.06);
      }

      if (v_is_skirt < 0.5 && v_is_side_face < 0.5) {
        final_color = mix(final_color, vec3(0.88, 0.99, 0.98), 0.14);

        // subtle toy-water fresnel lift
        float fresnel = pow(1.0 - saturate(dot(normal, view_dir)), 3.0);
        final_color = mix(final_color, vec3(0.96, 1.0, 0.99), fresnel * 0.14);

        // tiny specular-ish glint
        vec3 half_dir = normalize(light_dir + view_dir);
        float spec = pow(saturate(dot(normal, half_dir)), 24.0);
        final_color = mix(final_color, vec3(1.0, 1.0, 0.99), spec * 0.05);

        // subtle flow-like shimmer — same audit as grain/h_tone above: a
        // procedural sin() with no mip chain, so it gets the same
        // detail_fade treatment even though its own max strength (0.06) was
        // already modest.
        float flow_phase =
          sin(v_world_xy.x * 0.045 + v_world_xy.y * 0.020 + v_view_dist * 0.015) *
          sin(v_world_xy.y * 0.055 - v_world_xy.x * 0.018 + v_view_dist * 0.010);
        float shimmer = 0.5 + 0.5 * flow_phase;
        final_color = mix(final_color, vec3(0.95, 1.0, 0.99), shimmer * 0.06 * detail_fade);
      }
    }

    if (v_is_skirt < 0.5 && v_is_side_face < 0.5) {
      // fwidth()-based AA (inside getDynamicGrid) softens this line's own
      // edge, but it can't reduce how many *repeats* of the line fall inside
      // one pixel once cells minify enough — that's still a moiré, just a
      // softened one. 480 (the original fade-out distance) left a wide band
      // where the pattern was small enough to alias but not yet faded;
      // tightened to end by 220 to match the grain fade above, so all the
      // per-cell high-frequency detail (grid + grain) finishes fading out at
      // the same distance instead of the grid lingering past it.
      float dist_factor = smoothstep(0.0, 220.0, v_view_dist);
      float dist_fade = 1.0 - dist_factor;
      float current_width = mix(1.25, 0.4, dist_factor);
      float opacity_fade = dist_fade * dist_fade;
      float line_alpha = getDynamicGrid(v_box_uv, current_width);

      if (is_water) {
        line_alpha *= 0.08;
      }

      if (line_alpha > 0.01) {
        float grid_strength = line_alpha * u_border_opacity * opacity_fade;
        if (grid_strength > 0.001) {
          float lum = get_luminance(final_color);
          vec3 adaptive_color = mix(vec3(1.0), vec3(0.0), smoothstep(0.3, 0.7, lum));
          final_color = mix(final_color, adaptive_color, grid_strength);
        }
      }
    }

    // ---- distance pastel lift ----
    //
    // goal:
    // - soften far terrain into a tiny-diorama feel
    // - not true bloom, just a pastel atmospheric lift
    float pastel_factor = smoothstep(180.0, 720.0, v_view_dist);
    vec3 pastel_tint = vec3(1.02, 1.015, 1.01);

    if (is_water) {
      pastel_tint = vec3(1.01, 1.02, 1.025);
    }

    final_color = mix(final_color, final_color * pastel_tint, pastel_factor * 0.18);

    out_color = vec4(final_color, 1.0);

    #ifdef USE_FOG
      float fog_factor = 0.0;
      #ifdef FOG_EXP2
        fog_factor = 1.0 - exp( - fogDensity * fogDensity * vFogDepth * vFogDepth );
      #else
        fog_factor = smoothstep( fogNear, fogFar, vFogDepth );
      #endif

      if (is_water) {
        fog_factor *= 0.88;
      }

      out_color.rgb = mix( out_color.rgb, fogColor, fog_factor );
    #endif
  }
  `;
}

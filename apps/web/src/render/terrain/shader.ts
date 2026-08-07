// @pxl/web/render/terrain/shader.ts

import { GROUND_CH_KEYS, GroundChannelKey } from "@pxl/shared/bake/format.ts";
import * as meta from "./shader_meta.ts";
import { shadow_cfg } from "@/modules/world/render/config.ts";

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

    // ---- 0. LOD-boundary blend: which of this pair's two draws claims this cell ----
    //
    // u_blend_role/u_blend_t are only meaningful while this chunk is inside a
    // transition band (get_lod_guide's blend_active in lod.ts) — outside one,
    // chunk.tsx leaves role at 0 (finer) and t at 0, which makes core_visible
    // unconditionally true below, i.e. exactly today's behavior with one
    // extra always-true comparison.
    //
    // the partition key is the *coarser* cell's own (ix,iy) — the finer draw
    // (role 0) divides its own by 2 to land on the same key its 4 fine cells
    // share; the coarser draw (role 1) already IS at that granularity. Both
    // draws hash the identical key, so they agree on which side of u_blend_t
    // it falls on: exactly one of "this coarse cell" or "its 4 fine cells" is
    // visible for any given key at any given t, never both, never neither —
    // a stochastic swap instead of an alpha blend, which is what lets this
    // dither into a gradient across neighboring chunks (each with their own
    // t, but the same per-cell hash) instead of a flat see-through overlap.
    ivec2 blend_key = (u_blend_role < 0.5) ? (ivec2(ix_lod, iy_lod) / 2) : ivec2(ix_lod, iy_lod);
    float blend_r = fract(sin(dot(vec2(blend_key), vec2(12.9898, 78.233))) * 43758.5453123);
    bool core_visible = (u_blend_role < 0.5) ? (blend_r >= u_blend_t) : (blend_r < u_blend_t);

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
      h = (h00 + h01 + h10 + h11) / 4;
    }
    h = (h / int(u_lod_step)) * int(u_lod_step); // Z-quantization: apply the data-level LOD step (averaged sample still snaps to the LOD grid)

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

    float core_is_side_face = (position.z < 0.45) ? 1.0 : 0.0;

    vec3 scaled_local_pos = local_pos * visual_scale * 1.002;
    vec3 transformed = scaled_local_pos + instance_pos;
`;

// vertex shader
export const vert = /* glsl */ `
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

  uniform isampler2D u_heightmap;
  uniform float u_scale;
  uniform float u_row_stride;
  uniform float u_lod_step;
  // see position_core's step 0 above — chunk.tsx only sets these away from
  // their (0, 0) defaults on the two meshes it mounts while inside a
  // get_lod_guide blend band.
  uniform float u_blend_t;
  uniform float u_blend_role;

  void main() {
    ${position_core}

    v_uv = core_uv;
    v_box_uv = uv;
    v_is_skirt = core_is_skirt;
    v_is_side_face = core_is_side_face;
    v_local_pos = position;

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
  const ch_uniforms_decl = [
    ...GROUND_CH_KEYS.map((ch_key) => `uniform sampler2D ${meta.gen.U_CH(ch_key)};`),
    `uniform sampler2D u_edit_mask;`,
  ].join("\n  ");

  const ch_sampling_logic = GROUND_CH_KEYS
    .map((ch_key) => `float ${meta.gen.CH(ch_key)} = floor(texture(${meta.gen.U_CH(ch_key)}, v_uv).r * 255.0 + 0.5);`)
    .join("\n    ");
  const color_meta = meta.get_color_meta();
  const { color_uniforms_decl, color_mapping_funcs } = meta.build_color_mappings(color_meta);

  const base_land_uniform = meta.gen.U_COLOR(GroundChannelKey.LAND, "BARE");

  const ch_blending_logic = GROUND_CH_KEYS
    .map((ch_key) => {
      const ch_var_name: string = meta.gen.CH(ch_key);
      // STRUCTURE reuses the already-gated `is_structure` bool (declared
      // just above this block in the generated shader, see its own comment)
      // instead of re-reading the raw channel value — u_show_structures off
      // means a structure cell just never overrides final_color here, so it
      // falls back to whatever LAND/WATER/TRANSPORT channel already painted
      // underneath it, same as if the building had never been baked in.
      const cond = ch_key === GroundChannelKey.STRUCTURE ? "is_structure" : `${ch_var_name} > 0.5`;
      return `if (${cond}) final_color = ${meta.gen.FUNC_COLOR(ch_key)}(${ch_var_name});`;
    })
    .join("");

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
  ${color_uniforms_decl}

  uniform float u_border_opacity;
  uniform vec3 u_light_dir;
  // debug toggle (dev_panel.tsx's "건물" button, off by default) — see
  // is_structure below, the single point every structure-specific fragment
  // effect (blending, material grain, side-face tint, ...) already keys off,
  // so gating it there is enough to make hidden structure cells fall all the
  // way back through to whatever's underneath (land/water/transport) rather
  // than needing a separate check at each downstream usage.
  uniform bool u_show_structures;

  in vec2 v_uv;
  in vec2 v_box_uv;
  in float v_is_side_face;
  in float v_is_skirt;
  in float v_view_dist;
  in vec3 v_view_pos;
  in float v_surface_z;
  in vec2 v_world_xy;
  in vec3 v_local_pos;

  out vec4 out_color;

  ${color_mapping_funcs}

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

    ${ch_sampling_logic}
    bool is_water = ${meta.gen.CH(GroundChannelKey.WATER)} > 0.5;
    bool is_land = ${meta.gen.CH(GroundChannelKey.LAND)} > 0.5;
    bool is_structure = ${meta.gen.CH(GroundChannelKey.STRUCTURE)} > 0.5 && u_show_structures;
    bool is_transport = ${meta.gen.CH(GroundChannelKey.TRANSPORT)} > 0.5;
    vec3 final_color = ${base_land_uniform};
    ${ch_blending_logic}

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

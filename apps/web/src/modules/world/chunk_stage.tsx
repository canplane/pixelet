// @pxl/web/modules/world/chunk_stage.tsx

/**
 * Unified terrain/object renderer.
 *
 * One component represents one renderer-owned L2 XY column and all occupied
 * signed 64-high L2 bands beneath it. A temporary dense
 * signed-Z field is composited synchronously, converted to instance buffers,
 * and immediately returned to the scratch pool. In-frustum chunks retain a
 * 36 KiB top-surface summary for coarse LOD rebuilds, never the dense bands.
 */

import { createContext, memo, useContext, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useFrame, useThree } from "@react-three/fiber";
import * as THREE from "three";

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";
import { FROM_L3_IDX, L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { useRenderSettingsStore } from "@/render/settings_store.ts";
import { useMapStore } from "@/modules/map/store.ts";
import { light_cfg, rim_cfg } from "@/modules/world/environment/config.ts";
import { useAnchorRegistry } from "@/modules/world/anchor/context.tsx";
import { AnchorNode } from "@/modules/world/anchor/node.tsx";
import { l2_column_key_of, useObjectL2ColumnRevision } from "@/modules/world/objects/store.ts";
import { ObjectShadows } from "@/modules/world/objects/blob_shadow.tsx";
import { RENDER_RADIUS } from "@/modules/world/terrain/config.ts";
import { useTileStore } from "@/modules/world/terrain/store.ts";
import { CHEBYSHEV_DIST } from "@/modules/world/terrain/use_tile_fetcher.ts";
import { NO_LAYER } from "@/render/layer/pipeline/volume.ts";
import { in_range_chunks } from "@/render/chunk_range.ts";
import { compositor_cache_stats, with_temporary_composite } from "@/render/layer/pipeline/compositor.ts";
import { GROUND_LAYERS_SIDE, GROUND_LAYERS_TOP } from "@/render/texture_array.ts";
import {
  build_chunk_bucket,
  type BuiltChunkBucket,
  type BuiltChunkMesh,
  chunk_bounds,
  type ChunkBounds,
  type ChunkLodRole,
  type ChunkOffsets,
  mesh_bounds,
} from "@/render/layer/pipeline/chunk_bucket.ts";
import type { ChunkKey, WorldLayerStack } from "@/render/layer/pipeline/contract.ts";
import { coarse_instance_stats, occlusion_stats } from "@/render/layer/pipeline/chunk_mesher.ts";
import { type MeshRun, scan_all_column_runs } from "@/render/layer/pipeline/voxel_scan.ts";
import { get_lod_guide, get_lod_pair_visibility, type LODStep, step_lod_toward } from "@/render/lod.ts";

const LOD_RECHECK_INTERVAL: f64 = 0.15;
const LOD_WORK_BUDGET_MS: f64 = 4;
const LOD_MAX_CHUNKS_PER_FRAME: u32 = 8;

/**
 * ChunkInstanceMesh's useLayoutEffect — geometry creation + GPU buffer
 * upload (InstancedBufferAttribute) + bounds — runs in React's own render/
 * commit phase, outside chunk_stage.tsx's useFrame entirely. A commit()
 * only calls set_layout(); it doesn't force this work to happen inside the
 * useFrame timing window that scheduler_peak_stats covers, so a storm of
 * simultaneous LOD-transition mounts (many chunks blend_active at once)
 * could be expensive here without showing up in that measurement at all.
 */
const mesh_mount_stats = { total_ms: 0, count: 0, max_ms: 0 };
// geometry.dispose()/material.dispose() on unmount — a zoom-out flipping
// many chunks out of frustum at once disposes many meshes in one React
// commit, same shape of risk as the mount side above, never measured.
const mesh_unmount_stats = { total_ms: 0, count: 0, max_ms: 0 };

/**
 * Cumulative (since page load) LOD build timing, dev-only — no console.log
 * anywhere near this (a hot console during a build storm would itself skew
 * the measurement); just plain object writes each call, exposed read-only
 * via ChunkStage's existing __PXL_RENDER_STATS__ dev global. Inspect
 * from the console with e.g. `window.__PXL_RENDER_STATS__.lod_build`.
 */
const lod_build_stats = {
  scan_ms: 0,
  scan_count: 0,
  scan_max_ms: 0,
  scan_bands: 0,
  scan_bands_max: 0,
  build_ms: 0,
  build_count: 0,
  build_max_ms: 0,
  build_runs: 0,
  build_runs_max: 0,
  scan_deferred: 0,
  build_deferred: 0,
};

/**
 * A scan's cost tracks how many signed-Z bands the chunk actually has
 * (`volume.bands.length`, free right after compositing — no extra pass),
 * far more than it varies with anything else — profiled: a 41ms outlier
 * against a ~2ms mean, on a chunk with unusually many bands. Once this
 * session has scanned enough chunks to have a real mean cost-per-band, a
 * chunk whose estimated cost clears `LOD_WORK_BUDGET_MS *
 * SCAN_DEFER_FACTOR` gets deferred once (see scan_deferred_ref below) —
 * cooperative scheduling, not a worker: give some *other* pending chunk
 * (hopefully cheaper) a shot at this frame's single guaranteed slot
 * instead of unconditionally burning it on whichever chunk happened to
 * sort first.
 */
const SCAN_DEFER_FACTOR: f64 = 2;

/**
 * Session-peak (not cumulative-average) scheduler load, dev-only. A manual
 * `window.__PXL_RENDER_STATS__` read during a live stutter usually
 * lands a frame or two after the storm already passed (settling_lod/
 * pending_lod are back to 0 by the time you finish typing) — these track
 * the worst single frame actually seen instead, so the peak survives long
 * enough to read.
 */
const scheduler_peak_stats = {
  recheck_ms_max: 0,
  commit_ms_max: 0,
  frame_ms_max: 0,
  settling_lod_max: 0,
  blend_active_max: 0,
  pending_lod_total_max: 0,
  in_frustum_max: 0,
};

/**
 * Splits one commit() call's cost three ways, dev-only — commit_ms_max
 * (scheduler_peak_stats) hit 243ms with nothing else measured this session
 * (scan/build/mesh_mount) anywhere near that, so the remaining candidates
 * inside commit() itself — build_ref.current() (already covered above, but
 * cross-checked here too), chunk_bounds()'s plain-JS instance scan, and the
 * set_layout() call itself (React state update — normally just schedules
 * work, but if that scheduling isn't actually deferred here, this call is
 * where a synchronous re-render would show up) — need their own numbers.
 */
const commit_step_stats = {
  build_ms_max: 0,
  bounds_ms_max: 0,
  set_layout_ms_max: 0,
};

/**
 * The actual `gl.render(scene, camera)` call — three.js walking the scene
 * graph and issuing WebGL draw calls — happens in R3F's own frame loop,
 * *after* every useFrame callback above returns. Nothing in
 * scheduler_peak_stats/commit_step_stats covers it. Measured by wrapping
 * gl.render itself (see ChunkStage's effect below), not by tracing — every
 * DevTools Performance capture this session was dominated 80-90%+ by the
 * recording's own async-task-tracking overhead even in the single busiest
 * window, not real work, so a plain performance.now() wrapper is the only
 * trustworthy way left to see this number.
 */
const render_call_stats = { total_ms: 0, count: 0, max_ms: 0 };

const CHUNK_WORLD_SPAN: f64 = CHUNK_CELL_SPAN * U_PER_CELL;
const APPROXIMATE_CHUNK_RADIUS: f64 = CHUNK_WORLD_SPAN * 0.75;

function set_three_box(box: THREE.Box3, bounds: ChunkBounds): THREE.Box3 {
  box.min.set(bounds.min_x, bounds.min_y, bounds.min_z);
  box.max.set(bounds.max_x, bounds.max_y, bounds.max_z);
  return box;
}

function create_open_bottom_box(): THREE.BoxGeometry {
  const geo = new THREE.BoxGeometry(1, 1, 1);
  const idx = geo.getIndex();
  const bottom = geo.groups[geo.groups.length - 1];
  if (!idx || !bottom) return geo;
  geo.setIndex(new THREE.BufferAttribute((idx.array as Uint16Array).slice(0, bottom.start), 1));
  geo.groups = geo.groups.slice(0, -1);
  return geo;
}

const TERRAIN_BOX = create_open_bottom_box();
const OBJECT_BOX = new THREE.BoxGeometry(1, 1, 1);
const INSTANCE_RGB_ATTR = "color_ch_instance";
const INSTANCE_LAYER_TAPS_ATTR = "layer_idx_taps";
const INSTANCE_VISIBLE_ATTR = "lod_visible";

function inject_instance_shader(shader: {
  vertexShader: string;
  fragmentShader: string;
  uniforms: Record<string, THREE.IUniform>;
}, with_rim: boolean): void {
  shader.uniforms.u_ground_layers_top = { value: GROUND_LAYERS_TOP };
  shader.uniforms.u_ground_layers_side = { value: GROUND_LAYERS_SIDE };
  shader.uniforms.u_low_spec = { value: true };
  if (with_rim) {
    shader.uniforms.u_rim_color = { value: new THREE.Color(rim_cfg.COLOR) };
    // starts at 0, not rim_cfg.INTENSITY — ChunkInstanceMesh's own effect
    // (mirroring u_low_spec's) sets the real value from
    // render/settings_store.ts's show_rim_light once mounted, same
    // "compiled in unconditionally, toggled via a uniform" reasoning as
    // u_low_spec so flipping the setting never needs a shader recompile.
    shader.uniforms.u_rim_intensity = { value: 0 };
  }
  shader.vertexShader = shader.vertexShader
    .replace(
      "#include <common>",
      `#include <common>
      attribute vec3 ${INSTANCE_RGB_ATTR};
      attribute vec4 ${INSTANCE_LAYER_TAPS_ATTR};
      attribute float ${INSTANCE_VISIBLE_ATTR};
      varying vec3 v_${INSTANCE_RGB_ATTR};
      varying vec4 v_${INSTANCE_LAYER_TAPS_ATTR};
      varying float v_${INSTANCE_VISIBLE_ATTR};
      varying vec2 v_box_uv;
      varying float v_is_side;
      varying float v_side_x_dominant;
      varying vec3 v_local_pos;
      varying vec3 v_inst_scale;
      varying vec3 v_rim_view_pos;`,
    )
    .replace(
      "void main() {",
      `void main() {
        v_${INSTANCE_RGB_ATTR} = ${INSTANCE_RGB_ATTR};
        v_${INSTANCE_LAYER_TAPS_ATTR} = ${INSTANCE_LAYER_TAPS_ATTR};
        v_${INSTANCE_VISIBLE_ATTR} = ${INSTANCE_VISIBLE_ATTR};
        v_local_pos = position;
        v_is_side = (abs(normal.z) < 0.5) ? 1.0 : 0.0;
        v_side_x_dominant = (abs(normal.x) > 0.5) ? 1.0 : 0.0;
        // Real-size tile UV, not the box's own raw 0..1 face UV: write_scale_translation
        // (chunk_mesher.ts) bakes a merged run's real cell-unit extent straight into
        // instanceMatrix's diagonal (never a rotation, per that function's own contract),
        // so a run spanning many cells — any coarse-LOD box, or same-material cells merged
        // at any LOD — was one whole texture tile stretched across the entire face instead
        // of repeating once per real cell. Most visible on patterned side art (assets/cell/
        // textures/terrain/side_*.png): its layered-strata look read as a flat smear
        // once stretched that far. position is the unit box's own local [-0.5,0.5]
        // coordinate, so (position.axis + 0.5) * scale reconstructs a per-face 0..N
        // coordinate directly from it — sidesteps ever needing BoxGeometry's own per-face
        // UV axis convention. Side faces always put z (height) in the second component so
        // the strata art's up/down orientation stays consistent on all four side faces,
        // not flipped on two of them. GROUND_LAYERS_SIDE (texture_array.ts) is an untransformed
        // DataArrayTexture (flipY defaults false), so v=0 samples the source PNG's own top row
        // — (0.5 - position.z), not (position.z + 0.5), so the box's physical top (position.z
        // = +0.5) lands on v=0 and actually gets the image's top edge instead of its bottom
        // (e.g. side_bare.png's grass cap rendering at a cliff's foot instead of its crest).
        // RepeatWrapping (texture_array.ts) turns >1 straight into tiling.
        vec3 inst_scale = vec3(instanceMatrix[0].x, instanceMatrix[1].y, instanceMatrix[2].z);
        v_inst_scale = inst_scale;
        v_box_uv = v_is_side > 0.5
          ? (abs(normal.x) > 0.5
            ? vec2((position.y + 0.5) * inst_scale.y, (0.5 - position.z) * inst_scale.z)
            : vec2((position.x + 0.5) * inst_scale.x, (0.5 - position.z) * inst_scale.z))
          : vec2((position.x + 0.5) * inst_scale.x, (position.y + 0.5) * inst_scale.y);
        v_rim_view_pos = -(modelViewMatrix * instanceMatrix * vec4(position, 1.0)).xyz;`,
    );
  shader.fragmentShader = shader.fragmentShader
    .replace(
      "#include <common>",
      `#include <common>
      varying vec3 v_${INSTANCE_RGB_ATTR};
      varying vec4 v_${INSTANCE_LAYER_TAPS_ATTR};
      varying float v_${INSTANCE_VISIBLE_ATTR};
      varying vec2 v_box_uv;
      varying float v_is_side;
      varying float v_side_x_dominant;
      varying vec3 v_local_pos;
      varying vec3 v_inst_scale;
      varying vec3 v_rim_view_pos;
      uniform sampler2DArray u_ground_layers_top;
      uniform sampler2DArray u_ground_layers_side;
      uniform bool u_low_spec;
      ${
        with_rim
          ? `uniform vec3 u_rim_color;
      uniform float u_rim_intensity;`
          : ""
      }
      vec3 sample_layer_tap(float layer_idx, vec3 fallback_rgb) {
        if (layer_idx > ${NO_LAYER}.0 - 0.5) return fallback_rgb;
        return v_is_side > 0.5
          ? texture(u_ground_layers_side, vec3(v_box_uv, layer_idx)).rgb
          : texture(u_ground_layers_top, vec3(v_box_uv, layer_idx)).rgb;
      }
      // Fake edge bevel — MS2/Animal Crossing's own soft-corner read turned out to come
      // from rounded props/characters and soft shading, not genuinely rounded terrain
      // geometry (a real rounded-box mesh was prototyped and measured at 13-56x this
      // box's vertex count for real-time-viable segment counts — not worth it for what's
      // actually just a subtle edge highlight). Tried reshaping the lit *normal* near
      // edges first (toward normalize(local position), the same direction real corner-
      // rounding geometry would use) — measurably had zero visible effect, because this
      // block's own final color is emissive-driven (totalEmissiveRadiance = base_rgb *
      // FLOOR below), not real-light diffuse/specular, and the one normal-dependent term
      // that exists (rim light) both reads the untouched vNormal varying directly rather
      // than this local normal variable, and never runs for terrain (with_rim is object-only, see
      // create_material's own doc comment) — a normal tweak had nothing left to influence.
      // Darkening base_rgb directly, in the same thin edge band, works with this pipeline
      // instead of against it: a soft, direction-independent crease exactly where a real
      // chamfer would catch a shadow, with no dependency on which way anything is lit.
      //
      // That static darkening alone still reads flatter than the reference: real MS2
      // screenshots show edges catching an actual highlight that shifts as the camera
      // orbits, not just a fixed shadow line — the same view-angle-dependent cue
      // rim_cfg's own object-only rim light uses (pow(1-N.V, POWER), brightest at grazing
      // angles), just localized to the edge band instead of the whole silhouette, and
      // using the true rounded-corner direction (normalize(local position), the direction
      // real corner-rounding geometry would put its normal in) rather than the flat box
      // normal — so *this* narrow band is where a normal-based effect actually still
      // works, unlike the full-face attempt above: it's blended in as a separate additive
      // term below (outgoingLight), never routed back through the ignored lit normal variable.
      const float BEVEL_WORLD_WIDTH = 0.06;
      const float BEVEL_DARKEN = 0.72;
      const float EDGE_SHEEN_POWER = 2.0;
      const float EDGE_SHEEN_INTENSITY = 0.35;
      float edge_factor() {
        vec3 dist_to_edge = (0.5 - abs(v_local_pos)) * v_inst_scale;
        float d = v_is_side > 0.5
          ? (v_side_x_dominant > 0.5 ? min(dist_to_edge.y, dist_to_edge.z) : min(dist_to_edge.x, dist_to_edge.z))
          : min(dist_to_edge.x, dist_to_edge.y);
        return 1.0 - smoothstep(0.0, BEVEL_WORLD_WIDTH, d);
      }
      vec3 apply_fake_bevel(vec3 color) {
        return color * mix(1.0, BEVEL_DARKEN, edge_factor());
      }
      // Additive, not multiplied into base_rgb like apply_fake_bevel: a highlight brightens
      // regardless of what's under it, the same reasoning rim light itself is additive.
      vec3 edge_sheen() {
        vec3 corner_normal = normalize(v_local_pos);
        vec3 view_dir = normalize(v_rim_view_pos);
        float grazing = pow(1.0 - max(dot(corner_normal, view_dir), 0.0), EDGE_SHEEN_POWER);
        return vec3(grazing * edge_factor() * EDGE_SHEEN_INTENSITY);
      }`,
    )
    .replace("void main() {", `void main() { if (v_${INSTANCE_VISIBLE_ATTR} < 0.5) discard;`)
    .replace(
      "vec4 diffuseColor = vec4( diffuse, opacity );",
      `vec3 base_rgb = v_${INSTANCE_RGB_ATTR};
      if (!u_low_spec) {
        base_rgb = (
          sample_layer_tap(v_${INSTANCE_LAYER_TAPS_ATTR}.x, base_rgb) +
          sample_layer_tap(v_${INSTANCE_LAYER_TAPS_ATTR}.y, base_rgb) +
          sample_layer_tap(v_${INSTANCE_LAYER_TAPS_ATTR}.z, base_rgb) +
          sample_layer_tap(v_${INSTANCE_LAYER_TAPS_ATTR}.w, base_rgb)
        ) * 0.25;
      }
      base_rgb = apply_fake_bevel(base_rgb);
      vec4 diffuseColor = vec4(diffuse * base_rgb, opacity);`,
    )
    .replace("vec3 totalEmissiveRadiance = emissive;", `vec3 totalEmissiveRadiance = base_rgb * ${light_cfg.FLOOR};`)
    .replace(
      "vec3 outgoingLight = totalDiffuse + totalSpecular + totalEmissiveRadiance;",
      `vec3 outgoingLight = totalDiffuse + totalSpecular + totalEmissiveRadiance;
      outgoingLight += edge_sheen();`,
    );

  if (with_rim) {
    // vNormal is a standard varying the base meshphysical fragment shader
    // already declares — but vViewPosition is NOT: that one only exists in
    // the Lambert/Phong/Toon shader chunks (lights_lambert/phong/toon_pars_
    // fragment.glsl.js), never in meshphysical's own lights_physical_pars_
    // fragment.glsl.js, which MeshStandardMaterial/MeshPhysicalMaterial
    // both compile against. Referencing it here was an undeclared-
    // identifier shader compile error the whole time (rim silently never
    // applied, regardless of u_rim_intensity) — v_rim_view_pos above is our
    // own varying, computed in the vertex injection instead of assuming a
    // standard one that doesn't exist for this material. pow(1 - N.V,
    // POWER) is the classic fresnel/rim falloff: near 0 face-on (N.V close
    // to 1), rising toward 1 right at the silhouette (N.V close to 0) — see
    // rim_cfg's own doc comment for why this is object-only (with_rim is
    // only ever true for the OBJECT_BOX ChunkInstanceMesh, never
    // TERRAIN_BOX's).
    shader.fragmentShader = shader.fragmentShader.replace(
      "vec3 outgoingLight = totalDiffuse + totalSpecular + totalEmissiveRadiance;",
      `vec3 outgoingLight = totalDiffuse + totalSpecular + totalEmissiveRadiance;
      vec3 rim_view_dir = normalize(v_rim_view_pos);
      float rim = pow(1.0 - max(dot(normalize(vNormal), rim_view_dir), 0.0), ${rim_cfg.POWER});
      outgoingLight += u_rim_color * rim * u_rim_intensity;`,
    );
  }
}

type MountedChunkMesh = {
  built: BuiltChunkMesh;
  role: ChunkLodRole;
  attribute: THREE.InstancedBufferAttribute;
};
type ChunkMeshRegistry = { meshes: Set<MountedChunkMesh> };
type ChunkVisibilityDecision = { primary: boolean; secondary: boolean };

function apply_visibility(registry: ChunkMeshRegistry, decision: ChunkVisibilityDecision): void {
  for (const mounted of registry.meshes) {
    const value = (mounted.role === "primary" ? decision.primary : decision.secondary) ? 1 : 0;
    if (mounted.built.visibility[0] === value) continue;
    mounted.built.visibility.fill(value);
    mounted.attribute.needsUpdate = true;
  }
}

function create_geometry(built: BuiltChunkMesh, base: THREE.BoxGeometry): THREE.BufferGeometry {
  const geo = base.clone();
  geo.setAttribute(INSTANCE_RGB_ATTR, new THREE.InstancedBufferAttribute(built.rgb, 3));
  geo.setAttribute(INSTANCE_LAYER_TAPS_ATTR, new THREE.InstancedBufferAttribute(built.layer_taps, 4));
  geo.setAttribute(INSTANCE_VISIBLE_ATTR, new THREE.InstancedBufferAttribute(built.visibility, 1));
  return geo;
}

function create_material(with_rim: boolean): THREE.MeshStandardMaterial {
  const material = new THREE.MeshStandardMaterial({ roughness: 1, metalness: 0 });
  // Shadow map records back-face depth instead of front-face — the receiver
  // then tests against the far side of the caster instead of its own lit
  // surface, so normalBias no longer has to fight self-shadow acne and can
  // shrink toward zero without the shadow visibly detaching from its own
  // caster (environment/config.ts's BIAS_TEXELS).
  material.shadowSide = THREE.BackSide;
  // Material.customProgramCacheKey() defaults to "" — WebGLPrograms' cache
  // key is built only from the defines/parameters three.js itself tracks,
  // never from onBeforeCompile's own identity or its string output. The
  // terrain (with_rim=false) and object (with_rim=true) materials otherwise
  // hash identically and can share one cached WebGLProgram, silently
  // running whichever one compiled FIRST on both — the object mesh
  // rendering with terrain's rim-less shader, no error, no visible effect
  // no matter what u_rim_intensity is set to.
  material.customProgramCacheKey = () => with_rim ? "chunk-object-rim" : "chunk-terrain";
  material.onBeforeCompile = (shader) => {
    inject_instance_shader(shader, with_rim);
    shader.uniforms.u_low_spec.value = useRenderSettingsStore.getState().low_spec;
    if (with_rim) shader.uniforms.u_rim_intensity.value = useRenderSettingsStore.getState().show_rim_light ? rim_cfg.INTENSITY : 0;
    material.userData.shader = shader;
  };
  return material;
}

type ChunkInstanceMeshProps = {
  built: BuiltChunkMesh;
  role: ChunkLodRole;
  base: THREE.BoxGeometry;
  low_spec: boolean;
  // static per call site (TERRAIN_BOX vs OBJECT_BOX — see rim_cfg's own doc
  // comment) — not itself reactive, unlike show_rim_light below, which
  // toggles this mesh's own u_rim_intensity uniform once with_rim compiled
  // the term in at all.
  with_rim: boolean;
  show_rim_light: boolean;
  registry: ChunkMeshRegistry;
};

const ChunkInstanceMesh = memo(({ built, role, base, low_spec, with_rim, show_rim_light, registry }: ChunkInstanceMeshProps) => {
  const material = useMemo(() => create_material(with_rim), [with_rim]);
  const mesh = useMemo(() => {
    const m = new THREE.InstancedMesh(base.clone() as THREE.BufferGeometry, material, 1);
    // Unconditional: real cost is gated at the renderer level by Canvas's
    // own `shadows` prop (scene.tsx, driven by render/settings_store.ts's
    // show_shadow_map) — three.js doesn't compile any shadow-map shader
    // code, on a caster or a receiver, while gl.shadowMap.enabled is false,
    // so this flag alone costs nothing when the setting is off.
    m.castShadow = true;
    m.receiveShadow = true;
    return m;
  }, [base, material]);

  useEffect(() => () => {
    const t0 = import.meta.env.DEV ? performance.now() : 0;
    mesh.geometry.dispose();
    material.dispose();
    if (import.meta.env.DEV) {
      const unmount_ms = performance.now() - t0;
      mesh_unmount_stats.total_ms += unmount_ms;
      mesh_unmount_stats.count++;
      mesh_unmount_stats.max_ms = Math.max(mesh_unmount_stats.max_ms, unmount_ms);
    }
  }, [material, mesh]);
  useEffect(() => {
    const shader = material.userData.shader as { uniforms: Record<string, THREE.IUniform> } | undefined;
    if (shader) shader.uniforms.u_low_spec.value = low_spec;
  }, [material, low_spec]);
  useEffect(() => {
    if (!with_rim) return;
    const shader = material.userData.shader as { uniforms: Record<string, THREE.IUniform> } | undefined;
    if (shader) shader.uniforms.u_rim_intensity.value = show_rim_light ? rim_cfg.INTENSITY : 0;
  }, [material, with_rim, show_rim_light]);

  useLayoutEffect(() => {
    const mount_t0 = import.meta.env.DEV ? performance.now() : 0;
    const previous_geometry = mesh.geometry;
    const geometry = create_geometry(built, base);
    mesh.geometry = geometry;
    previous_geometry.dispose();
    mesh.instanceMatrix = new THREE.InstancedBufferAttribute(built.matrices, 16);
    mesh.instanceMatrix.needsUpdate = true;
    mesh.count = built.cnt;
    mesh.visible = built.cnt > 0;
    built.visibility.fill(role === "primary" ? 1 : 0);
    // Not mesh.computeBoundingBox()/computeBoundingSphere(): those walk
    // every instance through three.js's generic (rotation-capable)
    // transform, real cost on a chunk rebuild storm (profiled). Every box
    // this mesher emits is axis-aligned scale+translation only
    // (write_scale_translation's own contract), so mesh_bounds's direct
    // min/max scan is exact and far cheaper — same math chunk_bounds
    // already uses for the whole layout, just per mesh here.
    const bounds = mesh_bounds(built);
    if (bounds) {
      mesh.boundingBox = new THREE.Box3(
        new THREE.Vector3(bounds.min_x, bounds.min_y, bounds.min_z),
        new THREE.Vector3(bounds.max_x, bounds.max_y, bounds.max_z),
      );
      const center = mesh.boundingBox.getCenter(new THREE.Vector3());
      mesh.boundingSphere = new THREE.Sphere(center, center.distanceTo(mesh.boundingBox.max));
    } else {
      mesh.boundingBox = new THREE.Box3();
      mesh.boundingSphere = new THREE.Sphere();
    }
    const attribute = geometry.getAttribute(INSTANCE_VISIBLE_ATTR);
    if (import.meta.env.DEV) {
      const mount_ms = performance.now() - mount_t0;
      mesh_mount_stats.total_ms += mount_ms;
      mesh_mount_stats.count++;
      mesh_mount_stats.max_ms = Math.max(mesh_mount_stats.max_ms, mount_ms);
    }
    if (!(attribute instanceof THREE.InstancedBufferAttribute)) return;
    attribute.needsUpdate = true;
    const mounted = { built, role, attribute };
    registry.meshes.add(mounted);
    return () => void registry.meshes.delete(mounted);
  }, [base, built, mesh, registry, role]);

  return <primitive object={mesh} />;
});
ChunkInstanceMesh.displayName = "ChunkInstanceMesh";

type LodLayout = {
  primary: LODStep;
  secondary: LODStep | null;
  buckets: BuiltChunkBucket[];
  bounds: ChunkBounds | null;
  ready: boolean;
};

type ChunkLodControl = {
  blend_active: boolean;
  settling: boolean;
  in_frustum: boolean;
  distance2: f64;
  pending: boolean;
  recheck(camera: THREE.Camera, frustum: THREE.Frustum): void;
  commit(): boolean;
};

type LodSchedulerRegistry = { controls: Set<ChunkLodControl> };
const LodSchedulerContext = createContext<LodSchedulerRegistry | null>(null);

function use_lod_scheduler(): LodSchedulerRegistry {
  const scheduler = useContext(LodSchedulerContext);
  if (!scheduler) throw new Error("Chunk must be mounted under ChunkStage");
  return scheduler;
}

function blend_hash(key: string): f64 {
  let hash = 2166136261;
  for (let i = 0; i < key.length; i++) {
    hash ^= key.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0) / 0x1_0000_0000;
}

type InitialChunkView = { in_frustum: boolean; lod_step: LODStep };

function get_initial_chunk_view(
  camera: THREE.Camera,
  map_origin: IVec,
  page_idx: IVec,
  page_off: IVec,
): InitialChunkView {
  const canonical_origin = ivec.add(FROM_L3_IDX(page_idx), page_off);
  const local_origin = ivec.sub(canonical_origin, map_origin);
  const center = new THREE.Vector3(
    ivec.to_real(ivec.X(local_origin)) + CHUNK_WORLD_SPAN * 0.5,
    ivec.to_real(ivec.Y(local_origin)) + CHUNK_WORLD_SPAN * 0.5,
    0,
  );
  const projection = new THREE.Matrix4().multiplyMatrices(camera.projectionMatrix, camera.matrixWorldInverse);
  const frustum = new THREE.Frustum().setFromProjectionMatrix(projection);
  const sphere = new THREE.Sphere(center, APPROXIMATE_CHUNK_RADIUS);
  // A never-built chunk has no exact Z bounds yet. Preserve the camera's
  // projected chunk as a one-chunk safety guard; a full signed-Z column makes
  // nearly every horizontal chunk intersect a low-angle frustum.
  const camera_projection_inside = Math.abs(camera.position.x - center.x) <= CHUNK_WORLD_SPAN * 0.5 &&
    Math.abs(camera.position.y - center.y) <= CHUNK_WORLD_SPAN * 0.5;
  const in_frustum = camera_projection_inside || frustum.intersectsSphere(sphere);
  return {
    in_frustum,
    lod_step: in_frustum ? get_lod_guide(center.distanceTo(camera.position), 64).lod_step : 64,
  };
}

type ChunkProps = { page_idx: IVec; offset: ChunkOffsets; stack: WorldLayerStack };

const Chunk = memo(({ page_idx, offset, stack }: ChunkProps) => {
  const low_spec = useRenderSettingsStore((state) => state.low_spec);
  const show_rim_light = useRenderSettingsStore((state) => state.show_rim_light);
  const show_structures = useRenderSettingsStore((state) => state.show_structures);
  const tile = useTileStore((state) => state.get_tile(page_idx));
  const camera = useThree((state) => state.camera);
  const anchor_registry = useAnchorRegistry();
  const scheduler = use_lod_scheduler();
  const group_ref = useRef<THREE.Group>(null);
  const registry = useMemo<ChunkMeshRegistry>(() => ({ meshes: new Set() }), []);
  const page_off = useMemo(
    () => ivec.gather(BigInt(offset.base_x), BigInt(offset.base_y), 0n),
    [offset.base_x, offset.base_y],
  );
  const key = useMemo<ChunkKey>(() => ({ page_idx, page_off }), [page_idx, page_off]);
  const object_key = useMemo(() => l2_column_key_of(page_idx, page_off), [page_idx, page_off]);
  const objects_revision = useObjectL2ColumnRevision(object_key);

  const bucket_cache_ref = useRef(new Map<LODStep, BuiltChunkBucket>());
  // Every column's runs, scanned once from the live volume and reused
  // across every LOD tier and every LOD transition after that (cleared
  // alongside bucket_cache_ref below) — the composited dense bands still
  // live only for one synchronous pass, but this copy of their *results*
  // (plain MeshRun objects, not views into the volume) survives it.
  const column_runs_ref = useRef<MeshRun[][] | null>(null);
  // Set true the one time this chunk's scan gets deferred (see
  // SCAN_DEFER_FACTOR's doc comment) — cleared the moment it actually
  // scans, so a chunk can be deferred at most once, never starved forever
  // waiting for a calm frame that never comes.
  const scan_deferred_ref = useRef(false);
  // Same one-shot defer, for the accumulate/build step instead of the scan
  // — profiled: build_ref.current() itself (not just its scan half) spiked
  // to 39-243ms with scan_max_ms nowhere near that, most likely a LOD1
  // build (every run gets its own instance, no bucketing) landing near the
  // camera during a fast zoom. Separate from scan_deferred_ref: a chunk's
  // scan and build costs vary independently (scan tracks band count, build
  // tracks total run count).
  const build_deferred_ref = useRef(false);
  // null return means "a needed scan was just deferred, not built yet" —
  // distinct from an empty array (built, genuinely nothing to draw).
  const build_ref = useRef<(primary: LODStep, secondary: LODStep | null) => BuiltChunkBucket[] | null>(() => []);
  build_ref.current = (primary, secondary) => {
    const steps = secondary === null || secondary === primary ? [primary] : [primary, secondary];
    const missing = steps.filter((step) => !bucket_cache_ref.current.has(step));

    if (missing.length > 0) {
      if (!column_runs_ref.current) {
        let deferred = false;
        const scan_t0 = performance.now();
        with_temporary_composite(stack, key, (volume) => {
          const band_count = volume.bands.length;
          const avg_bands = lod_build_stats.scan_count > 0 ? lod_build_stats.scan_bands / lod_build_stats.scan_count : 0;
          const avg_ms = lod_build_stats.scan_count > 0 ? lod_build_stats.scan_ms / lod_build_stats.scan_count : 0;
          const estimated_ms = avg_bands > 0 ? avg_ms * (band_count / avg_bands) : 0;
          if (!scan_deferred_ref.current && estimated_ms > LOD_WORK_BUDGET_MS * SCAN_DEFER_FACTOR) {
            scan_deferred_ref.current = true;
            deferred = true;
            lod_build_stats.scan_deferred++;
            return;
          }
          scan_deferred_ref.current = false;
          column_runs_ref.current = scan_all_column_runs(volume);
          lod_build_stats.scan_bands += band_count;
          lod_build_stats.scan_bands_max = Math.max(lod_build_stats.scan_bands_max, band_count);
        });
        if (deferred) return null;
        const scan_ms = performance.now() - scan_t0;
        lod_build_stats.scan_ms += scan_ms;
        lod_build_stats.scan_count++;
        lod_build_stats.scan_max_ms = Math.max(lod_build_stats.scan_max_ms, scan_ms);
      }
      const column_runs = column_runs_ref.current;
      if (column_runs) {
        let total_runs = 0;
        for (const list of column_runs) total_runs += list.length;
        const avg_runs = lod_build_stats.build_count > 0 ? lod_build_stats.build_runs / lod_build_stats.build_count : 0;
        const avg_build_ms = lod_build_stats.build_count > 0 ? lod_build_stats.build_ms / lod_build_stats.build_count : 0;
        const estimated_ms = avg_runs > 0 ? avg_build_ms * (total_runs / avg_runs) * missing.length : 0;
        if (!build_deferred_ref.current && estimated_ms > LOD_WORK_BUDGET_MS * SCAN_DEFER_FACTOR) {
          build_deferred_ref.current = true;
          lod_build_stats.build_deferred++;
          return null;
        }
        build_deferred_ref.current = false;

        const build_t0 = performance.now();
        for (const step of missing) {
          bucket_cache_ref.current.set(step, build_chunk_bucket(column_runs, step, "primary"));
        }
        const build_ms = performance.now() - build_t0;
        lod_build_stats.build_ms += build_ms;
        lod_build_stats.build_count++;
        lod_build_stats.build_max_ms = Math.max(lod_build_stats.build_max_ms, build_ms);
        lod_build_stats.build_runs += total_runs;
        lod_build_stats.build_runs_max = Math.max(lod_build_stats.build_runs_max, total_runs);
      }
    }

    // Built buckets for LOD tiers outside the current primary/secondary are
    // kept, not evicted here — profiled: a chunk build storm during a pan
    // repeatedly revisits the same tier (LOD hunting back and forth around
    // a threshold), and rebuilding one from column_runs isn't free (a full
    // res^2 * lod_step^2 accumulate_run sweep even with runs pre-scanned).
    // bucket_cache_ref is only actually cleared on frustum-exit or a source
    // revision change (below), at most LOD_STEP_BY_LEVEL.length (7) small
    // entries retained per chunk — cheap next to column_runs_ref.
    return steps
      .map((step) => bucket_cache_ref.current.get(step))
      .filter((bucket): bucket is BuiltChunkBucket => bucket !== undefined)
      .map((bucket) => ({
        ...bucket,
        role: (bucket.lod_step === primary ? "primary" : "secondary") as ChunkLodRole,
      }))
      .sort((a, b) => a.lod_step - b.lod_step);
  };

  const initial_view = useMemo(
    () => get_initial_chunk_view(camera, anchor_registry.cur_map_origin, page_idx, page_off),
    [anchor_registry, camera, page_idx, page_off],
  );

  const [layout, set_layout] = useState<LodLayout>(() => {
    if (!initial_view.in_frustum) {
      return { primary: 64, secondary: null, buckets: [], bounds: null, ready: false };
    }
    const buckets = build_ref.current(initial_view.lod_step, null);
    if (buckets === null) {
      // Scan deferred on the very first build — start not-ready (same
      // shape as an out-of-frustum chunk) so the first recheck() sets
      // pending and retries, instead of rendering nothing forever.
      return { primary: initial_view.lod_step, secondary: null, buckets: [], bounds: null, ready: false };
    }
    const initial: LodLayout = {
      primary: initial_view.lod_step,
      secondary: null,
      buckets,
      bounds: chunk_bounds(buckets),
      ready: true,
    };
    return initial;
  });
  const layout_ref = useRef(layout);
  layout_ref.current = layout;
  const desired_ref = useRef<{ primary: LODStep; secondary: LODStep | null }>({
    primary: initial_view.lod_step,
    secondary: null,
  });
  const source_ref = useRef({ tile, objects_revision, show_structures });

  // A source revision rebuilds the currently displayed LOD, not the entire
  // visible world. Comparing the actual source tuple also prevents React
  // StrictMode's development-only effect replay from rebuilding every chunk.
  useEffect(() => {
    const previous = source_ref.current;
    if (
      Object.is(previous.tile, tile) && previous.objects_revision === objects_revision &&
      previous.show_structures === show_structures
    ) return;
    source_ref.current = { tile, objects_revision, show_structures };
    const current = layout_ref.current;
    bucket_cache_ref.current.clear();
    column_runs_ref.current = null;
    if (!current.ready) {
      if (current.bounds !== null) {
        const invalidated = { ...current, bounds: null };
        layout_ref.current = invalidated;
        set_layout(invalidated);
      }
      return;
    }
    const buckets = build_ref.current(current.primary, current.secondary);
    if (buckets === null) {
      // Deferred — keep whatever's currently showing, just mark not-ready
      // so the next recheck() sets pending and retries.
      if (current.ready) {
        const invalidated = { ...current, ready: false };
        layout_ref.current = invalidated;
        set_layout(invalidated);
      }
      return;
    }
    const next = { ...current, buckets, bounds: chunk_bounds(buckets), ready: true };
    layout_ref.current = next;
    set_layout(next);
  }, [tile, objects_revision, show_structures]);

  const control = useMemo<ChunkLodControl>(() => {
    const center = new THREE.Vector3();
    const local_box = new THREE.Box3();
    const world_box = new THREE.Box3();
    const sphere = new THREE.Sphere(center, APPROXIMATE_CHUNK_RADIUS);
    const stable_hash = blend_hash(`${page_idx.toString()}:${offset.key}`);
    return {
      blend_active: true,
      settling: false,
      in_frustum: initial_view.in_frustum,
      distance2: Infinity,
      pending: false,
      recheck(camera, frustum) {
        const group = group_ref.current;
        if (!group) return;
        const current = layout_ref.current;
        const bounds = current.bounds;
        if (bounds === null) {
          center.set(CHUNK_WORLD_SPAN * 0.5, CHUNK_WORLD_SPAN * 0.5, 0).applyMatrix4(group.matrixWorld);
        } else {
          set_three_box(local_box, bounds);
          local_box.getCenter(center).applyMatrix4(group.matrixWorld);
        }
        this.distance2 = center.distanceToSquared(camera.position);
        if (bounds === null) {
          const camera_projection_inside = Math.abs(camera.position.x - center.x) <= CHUNK_WORLD_SPAN * 0.5 &&
            Math.abs(camera.position.y - center.y) <= CHUNK_WORLD_SPAN * 0.5;
          this.in_frustum = camera_projection_inside || frustum.intersectsSphere(sphere);
        } else {
          world_box.copy(local_box).applyMatrix4(group.matrixWorld);
          const camera_projection_inside = camera.position.x >= world_box.min.x && camera.position.x <= world_box.max.x &&
            camera.position.y >= world_box.min.y && camera.position.y <= world_box.max.y;
          this.in_frustum = camera_projection_inside || frustum.intersectsBox(world_box);
        }
        group.visible = this.in_frustum;

        if (!this.in_frustum) {
          // Do not let GPU buffers or derived surfaces accumulate as the
          // camera visits the world. An off-screen component keeps only its
          // source key and rebuilds if it re-enters the frustum.
          desired_ref.current = { primary: 64, secondary: null };
          this.pending = current.ready;
          this.blend_active = false;
          this.settling = false;
          for (const step of bucket_cache_ref.current.keys()) {
            if (step !== 64) bucket_cache_ref.current.delete(step);
          }
          apply_visibility(registry, { primary: true, secondary: false });
          return;
        }
        const guide = get_lod_guide(Math.sqrt(this.distance2), current.primary);
        // An unloaded chunk has nothing on screen to transition from. Build
        // its correct tier immediately instead of flashing 32 -> 16 -> target
        // merely because the memory-saving empty state uses step 64.
        const primary = current.ready ? step_lod_toward(current.primary, guide.lod_step) : guide.lod_step;
        this.settling = primary !== guide.lod_step;
        const secondary = !this.settling && guide.blend_active && guide.blend_finer_step !== null && guide.blend_coarser_step !== null
          ? (primary === guide.blend_finer_step ? guide.blend_coarser_step : guide.blend_finer_step)
          : null;
        desired_ref.current = { primary, secondary };
        this.pending = !current.ready || current.primary !== primary || current.secondary !== secondary;
        this.blend_active = !this.settling && guide.blend_active;

        if (this.pending || !this.blend_active || guide.blend_finer_step === null || guide.blend_coarser_step === null) {
          apply_visibility(registry, { primary: true, secondary: false });
          return;
        }
        const pair = get_lod_pair_visibility(stable_hash, guide.coarse_fraction);
        const primary_is_finer = current.primary === guide.blend_finer_step;
        apply_visibility(registry, {
          primary: primary_is_finer ? pair.finer : pair.coarser,
          secondary: primary_is_finer ? pair.coarser : pair.finer,
        });
      },
      commit() {
        if (!this.pending) return false;
        if (!this.in_frustum) {
          bucket_cache_ref.current.clear();
          column_runs_ref.current = null;
          // Keep the tiny measured AABB after releasing GPU/surface data so a
          // later polar change can re-enter using real geometry, not z=0.
          const empty: LodLayout = {
            primary: 64,
            secondary: null,
            buckets: [],
            bounds: layout_ref.current.bounds,
            ready: false,
          };
          layout_ref.current = empty;
          set_layout(empty);
          this.pending = false;
          return true;
        }
        const desired = desired_ref.current;
        const build_t0 = import.meta.env.DEV ? performance.now() : 0;
        const buckets = build_ref.current(desired.primary, desired.secondary);
        if (import.meta.env.DEV) {
          commit_step_stats.build_ms_max = Math.max(commit_step_stats.build_ms_max, performance.now() - build_t0);
        }
        if (buckets === null) {
          // A needed scan was deferred to a calmer frame — stay pending
          // (retried next tick) and leave whatever's currently on screen
          // alone rather than popping it to empty.
          return false;
        }
        const bounds_t0 = import.meta.env.DEV ? performance.now() : 0;
        const bounds = chunk_bounds(buckets);
        if (import.meta.env.DEV) {
          commit_step_stats.bounds_ms_max = Math.max(commit_step_stats.bounds_ms_max, performance.now() - bounds_t0);
        }
        const next: LodLayout = {
          primary: desired.primary,
          secondary: desired.secondary,
          buckets,
          bounds,
          ready: true,
        };
        layout_ref.current = next;
        const set_layout_t0 = import.meta.env.DEV ? performance.now() : 0;
        set_layout(next);
        if (import.meta.env.DEV) {
          commit_step_stats.set_layout_ms_max = Math.max(commit_step_stats.set_layout_ms_max, performance.now() - set_layout_t0);
        }
        this.pending = false;
        return true;
      },
    };
  }, [offset.key, initial_view.in_frustum, page_idx, registry]);

  useEffect(() => {
    scheduler.controls.add(control);
    return () => void scheduler.controls.delete(control);
  }, [scheduler, control]);

  return (
    <group
      ref={group_ref}
      position={[offset.base_x * U_PER_CELL, offset.base_y * U_PER_CELL, 0]}
      name={`chunk/${offset.key}`}
    >
      {layout.buckets.map((bucket) => (
        <group key={bucket.role} name={`${bucket.role}/lod/${bucket.lod_step}`}>
          {bucket.terrain.cnt > 0 && <ChunkInstanceMesh built={bucket.terrain} role={bucket.role} base={TERRAIN_BOX} low_spec={low_spec} with_rim={false} show_rim_light={false} registry={registry} />}
          {bucket.objects.cnt > 0 && <ChunkInstanceMesh built={bucket.objects} role={bucket.role} base={OBJECT_BOX} low_spec={low_spec} with_rim show_rim_light={show_rim_light} registry={registry} />}
        </group>
      ))}
    </group>
  );
});
Chunk.displayName = "Chunk";

type PageChunksProps = { page_idx: IVec; stack: WorldLayerStack; range_anchor: IVec };

const PageChunks = memo(({ page_idx, stack, range_anchor }: PageChunksProps) => {
  const page_base = FROM_L3_IDX(page_idx);
  const show_blob_shadow = useRenderSettingsStore((state) => state.show_blob_shadow);
  const offsets = useMemo(
    () => in_range_chunks(page_base, range_anchor),
    [page_base, range_anchor],
  );
  return (
    <AnchorNode i={page_base}>
      <group name={`chunk-stage-page/${ivec.repr(page_idx)}`}>
        {offsets.map((offset) => <Chunk key={offset.key} page_idx={page_idx} offset={offset} stack={stack} />)}
        {show_blob_shadow && offsets.map((offset) => (
          <group key={`shadow/${offset.key}`} position={[offset.base_x * U_PER_CELL, offset.base_y * U_PER_CELL, 0]}>
            <ObjectShadows
              page_idx={page_idx}
              page_off={ivec.gather(BigInt(offset.base_x), BigInt(offset.base_y), 0n)}
            />
          </group>
        ))}
      </group>
    </AnchorNode>
  );
});
PageChunks.displayName = "PageChunks";

export type ChunkStageProps = { stack: WorldLayerStack };

export const ChunkStage = memo(({ stack }: ChunkStageProps) => {
  const map_center = useMapStore((state) => state.map_center);
  const tile_map = useTileStore((state) => state.tile_map);
  const scheduler = useMemo<LodSchedulerRegistry>(() => ({ controls: new Set() }), []);
  const lod_elapsed = useRef(LOD_RECHECK_INTERVAL);
  const frustum = useMemo(() => new THREE.Frustum(), []);
  const { gl } = useThree();

  useEffect(() => {
    if (!import.meta.env.DEV) return;
    const original_render = gl.render.bind(gl);
    gl.render = (scene, camera) => {
      const t0 = performance.now();
      original_render(scene, camera);
      const render_ms = performance.now() - t0;
      render_call_stats.total_ms += render_ms;
      render_call_stats.count++;
      render_call_stats.max_ms = Math.max(render_call_stats.max_ms, render_ms);
    };
    return () => {
      gl.render = original_render;
    };
  }, [gl]);
  const projection = useMemo(() => new THREE.Matrix4(), []);

  useFrame((state, delta) => {
    const frame_t0 = import.meta.env.DEV ? performance.now() : 0;
    const controls = Array.from(scheduler.controls);
    if (controls.length === 0) return;
    lod_elapsed.current += delta;
    const full_recheck = lod_elapsed.current >= LOD_RECHECK_INTERVAL;
    if (full_recheck) lod_elapsed.current = 0;
    projection.multiplyMatrices(state.camera.projectionMatrix, state.camera.matrixWorldInverse);
    frustum.setFromProjectionMatrix(projection);

    const recheck_t0 = import.meta.env.DEV ? performance.now() : 0;
    // Same cooperative shape as the commit loop below: the first eligible
    // control this frame always runs (forward progress guaranteed even on
    // a slow frame), the rest are time-boxed. Profiled: during a fast zoom,
    // settling_lod/blend_active can both spike into the dozens at once, and
    // recheck()-ing every one of them every single frame (previously
    // unconditional) cost up to ~23ms by itself — a stale blend/settling
    // flag for one skipped frame is a much smaller visual cost than that.
    const recheck_deadline = performance.now() + LOD_WORK_BUDGET_MS;
    let blend_active = false;
    let rechecked: u32 = 0;
    for (const control of controls) {
      const needs_recheck = full_recheck || control.blend_active || control.settling;
      if (needs_recheck && (rechecked === 0 || performance.now() < recheck_deadline)) {
        control.recheck(state.camera, frustum);
        rechecked++;
      }
      blend_active ||= control.blend_active;
    }
    const recheck_ms = import.meta.env.DEV ? performance.now() - recheck_t0 : 0;

    const pending_total = controls.filter((control) => control.pending);
    // Visible builds/refinements win. Pending work outside the frustum only
    // drops a formerly visible chunk's derived surface and GPU buffers.
    const pending = pending_total.sort((a, b) => Number(b.in_frustum) - Number(a.in_frustum) || a.distance2 - b.distance2);
    const deadline = performance.now() + LOD_WORK_BUDGET_MS;
    const commit_t0 = import.meta.env.DEV ? performance.now() : 0;
    let committed: u32 = 0;
    for (const control of pending) {
      if (committed >= LOD_MAX_CHUNKS_PER_FRAME || (committed > 0 && performance.now() >= deadline)) break;
      if (control.commit()) committed++;
    }
    const commit_ms = import.meta.env.DEV ? performance.now() - commit_t0 : 0;
    if (import.meta.env.DEV) {
      const settling_lod = controls.filter((control) => control.settling).length;
      const blend_active_cnt = controls.filter((control) => control.blend_active).length;
      const in_frustum_cnt = controls.filter((control) => control.in_frustum).length;
      const pending_lod_total_cnt = pending_total.length - committed;
      const frame_ms = performance.now() - frame_t0;
      scheduler_peak_stats.recheck_ms_max = Math.max(scheduler_peak_stats.recheck_ms_max, recheck_ms);
      scheduler_peak_stats.commit_ms_max = Math.max(scheduler_peak_stats.commit_ms_max, commit_ms);
      scheduler_peak_stats.frame_ms_max = Math.max(scheduler_peak_stats.frame_ms_max, frame_ms);
      scheduler_peak_stats.settling_lod_max = Math.max(scheduler_peak_stats.settling_lod_max, settling_lod);
      scheduler_peak_stats.blend_active_max = Math.max(scheduler_peak_stats.blend_active_max, blend_active_cnt);
      scheduler_peak_stats.pending_lod_total_max = Math.max(scheduler_peak_stats.pending_lod_total_max, pending_lod_total_cnt);
      scheduler_peak_stats.in_frustum_max = Math.max(scheduler_peak_stats.in_frustum_max, in_frustum_cnt);

      const debug_stats = {
        chunks: controls.length,
        in_frustum: in_frustum_cnt,
        pending_lod: pending.length - committed,
        pending_lod_total: pending_lod_total_cnt,
        blend_active: blend_active_cnt,
        settling_lod,
        draw_calls: state.gl.info.render.calls,
        triangles: state.gl.info.render.triangles,
        geometries: state.gl.info.memory.geometries,
        textures: state.gl.info.memory.textures,
        compositor: compositor_cache_stats(),
        lod_build: { ...lod_build_stats },
        coarse_instances: { ...coarse_instance_stats },
        occlusion: { ...occlusion_stats },
        scheduler_peak: { ...scheduler_peak_stats },
        mesh_mount: { ...mesh_mount_stats },
        mesh_unmount: { ...mesh_unmount_stats },
        commit_step: { ...commit_step_stats },
        render_call: { ...render_call_stats },
      };
      (globalThis as typeof globalThis & { __PXL_RENDER_STATS__?: unknown }).__PXL_RENDER_STATS__ = debug_stats;
      state.gl.domElement.dataset.pixeletRenderStats = JSON.stringify(debug_stats);
    }
    if (committed > 0 || pending.length > committed || blend_active) state.invalidate();
  });

  const center_page_idx = map_center === null ? null : L3_IDX(map_center.i);
  const center_page_key = center_page_idx?.toString() ?? null;
  const tiles = useMemo(() => Array.from(tile_map.keys()), [tile_map]);
  const visible_pages = useMemo(() => {
    if (center_page_idx === null) return tiles;
    return tiles
      .map((page_idx) => ({ page_idx, dist: CHEBYSHEV_DIST(ivec.sub(page_idx, center_page_idx)) }))
      .filter(({ dist }) => dist <= RENDER_RADIUS)
      .sort((a, b) => a.dist - b.dist)
      .map(({ page_idx }) => page_idx);
  }, [tiles, center_page_key]);

  const anchor_x = map_center === null ? null : ivec.X(map_center.i) & ~63n;
  const anchor_y = map_center === null ? null : ivec.Y(map_center.i) & ~63n;
  const range_anchor_key = anchor_x === null ? null : `${anchor_x},${anchor_y}`;
  const range_anchor = useMemo(
    () => anchor_x === null || anchor_y === null ? null : ivec.gather(anchor_x, anchor_y, 0n),
    [range_anchor_key],
  );

  if (range_anchor === null) return null;
  return (
    <LodSchedulerContext.Provider value={scheduler}>
      <group name="chunk-stage">
        {visible_pages.map((page_idx) => <PageChunks key={page_idx.toString()} page_idx={page_idx} stack={stack} range_anchor={range_anchor} />)}
      </group>
    </LodSchedulerContext.Provider>
  );
});
ChunkStage.displayName = "ChunkStage";

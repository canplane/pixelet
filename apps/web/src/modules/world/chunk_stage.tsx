// @pxl/web/modules/world/chunk_stage.tsx

/**
 * Unified terrain/object/decor renderer.
 *
 * One Chunk component represents one renderer-owned L2 XY column and all
 * occupied signed 64-high L2 bands beneath it — AND, as of 2026-09-05, every
 * decor kind's own instances for that same column too. A temporary dense
 * signed-Z field is composited synchronously, converted to a baked mesh, and
 * immediately returned to the scratch pool. In-frustum chunks retain a
 * 36 KiB top-surface summary for coarse LOD rebuilds, never the dense bands.
 *
 * Decor used to be a fully separate, independently-scheduled component tree
 * (decor_layer.tsx's own ChunkDecor/PageDecor/DecorLayer, one registrant per
 * (chunk, decor kind) in the shared LodSchedulerRegistry below) — retired
 * once Chunk absorbed decor's own build into its own commit(), the fix for
 * the documented terrain/decor ready-state desync (decor could render a
 * frame before its own co-located terrain). See decor_layer.tsx's own top
 * doc comment and discussions/20260905-tier1-geometry-baking-implemented.md
 * (and its own same-day follow-up) for the fuller account.
 */

import { createContext, memo, type ReactNode, useContext, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useFrame, useThree } from "@react-three/fiber";
import * as THREE from "three";

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";
import { FROM_L3_IDX, L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { useMapStore } from "@/modules/map/store.ts";
import { light_cfg, rim_cfg } from "@/modules/world/environment/config.ts";
import { useAnchorRegistry } from "@/modules/world/anchor/context.tsx";
import { AnchorNode } from "@/modules/world/anchor/node.tsx";
import { l2_column_key_of, useDecorL2ColumnRevision, useObjectL2ColumnRevision } from "@/modules/world/objects/store.ts";
import { glsl_float, inject_rim_light_shader } from "@/modules/world/render/rim_light_shader.ts";
import { apply_visibility, type ChunkMeshRegistry, type MountedLodMesh } from "@/render/lod_visibility.ts";
import { RENDER_RADIUS } from "@/modules/world/terrain/config.ts";
import { useTileStore } from "@/modules/world/terrain/store.ts";
import { CHEBYSHEV_DIST } from "@/modules/world/terrain/use_tile_fetcher.ts";
import { NO_LAYER } from "@/render/layer/pipeline/volume.ts";
import { in_range_chunks } from "@/render/chunk_range.ts";
import { compositor_cache_stats, with_temporary_composite } from "@/render/layer/pipeline/compositor.ts";
import { GROUND_LAYERS_SIDE, GROUND_LAYERS_TOP } from "@/render/tex_array.ts";
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
import { get_lod_guide, get_lod_pair_visibility, type LodStep, step_lod_toward } from "@/render/lod.ts";
import { APPROXIMATE_CHUNK_RADIUS, chunk_distance_to_camera, estimate_initial_chunk_view, update_frustum } from "@/render/chunk_lod.ts";
import {
  build_decor_instances,
  type DecorBundleAssets,
  type DecorKindBundle,
  DecorVariantMesh,
  type ModelInstanceTransformList,
  useAllDecorBundleAssets,
} from "@/modules/world/decor/decor_layer.tsx";

const LOD_RECHECK_INTERVAL: f64 = 0.15;
const LOD_WORK_BUDGET_MS: f64 = 4;
const LOD_MAX_CHUNKS_PER_FRAME: u32 = 8;

/**
 * ChunkMesh's useLayoutEffect — geometry creation + GPU buffer upload
 * (BufferAttribute) + bounds — runs in React's own render/commit phase,
 * outside chunk_stage.tsx's useFrame entirely. A commit()
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

function set_three_box(box: THREE.Box3, bounds: ChunkBounds): THREE.Box3 {
  box.min.set(bounds.min_x, bounds.min_y, bounds.min_z);
  box.max.set(bounds.max_x, bounds.max_y, bounds.max_z);
  return box;
}

// Per-vertex attribute names for the baked chunk mesh (chunk_mesher.ts) —
// replaces what used to be per-INSTANCE attributes read through
// instanceMatrix; every vertex now already carries its own real
// position/normal, so these are the only extra material/shading inputs the
// shader needs (see inject_chunk_shader's own doc comment).
const RGB_ATTR = "a_rgb";
const LAYER_TAPS_ATTR = "a_layer_taps";
const VISIBLE_ATTR = "a_visible";
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
function create_geometry(built: BuiltChunkMesh): THREE.BufferGeometry {
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

type ChunkMeshProps = {
  built: BuiltChunkMesh;
  role: ChunkLodRole;
  low_spec: boolean;
  show_rim_light: boolean;
  registry: ChunkMeshRegistry;
};

/** One real, non-instanced THREE.Mesh per chunk per LOD role — every cube
 * (terrain and solid objects alike) baked directly into its own geometry
 * (chunk_mesher.ts), not a shared box restyled per instance. */
const ChunkMesh = memo(({ built, role, low_spec, show_rim_light, registry }: ChunkMeshProps) => {
  const material = useMemo(() => create_material(), []);
  const mesh = useMemo(() => {
    const m = new THREE.Mesh(new THREE.BufferGeometry(), material);
    // Unconditional: real cost is gated at the renderer level by Canvas's
    // own `shadows` prop (scene.tsx, driven by app/settings/settings_store.ts's
    // show_shadow_map) — three.js doesn't compile any shadow-map shader
    // code, on a caster or a receiver, while gl.shadowMap.enabled is false,
    // so this flag alone costs nothing when the setting is off.
    m.castShadow = true;
    m.receiveShadow = true;
    return m;
  }, [material]);

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
    const shader = material.userData.shader as { uniforms: Record<string, THREE.IUniform> } | undefined;
    if (shader) shader.uniforms.u_rim_intensity.value = show_rim_light ? rim_cfg.INTENSITY : 0;
  }, [material, show_rim_light]);

  useLayoutEffect(() => {
    const mount_t0 = import.meta.env.DEV ? performance.now() : 0;
    const previous_geometry = mesh.geometry;
    const geometry = create_geometry(built);
    mesh.geometry = geometry;
    previous_geometry.dispose();
    mesh.visible = built.vertex_cnt > 0;
    built.visibility.fill(role === "primary" ? 1 : 0);
    // Not geometry.computeBoundingBox()/computeBoundingSphere(): those walk
    // every vertex through three.js's own generic accumulation, real cost on
    // a chunk rebuild storm (profiled, back when this walked per-instance —
    // still true per-vertex). mesh_bounds's direct min/max scan over the
    // already-baked real positions is exact and far cheaper — same math
    // chunk_bounds already uses for the whole layout, just per mesh here.
    // A plain (non-Instanced) Mesh has no bounding fields of its own — those
    // live on the geometry, unlike InstancedMesh's own instance-level ones.
    const bounds = mesh_bounds(built);
    if (bounds) {
      geometry.boundingBox = new THREE.Box3(
        new THREE.Vector3(bounds.min_x, bounds.min_y, bounds.min_z),
        new THREE.Vector3(bounds.max_x, bounds.max_y, bounds.max_z),
      );
      const center = geometry.boundingBox.getCenter(new THREE.Vector3());
      geometry.boundingSphere = new THREE.Sphere(center, center.distanceTo(geometry.boundingBox.max));
    } else {
      geometry.boundingBox = new THREE.Box3();
      geometry.boundingSphere = new THREE.Sphere();
    }
    const attribute = geometry.getAttribute(VISIBLE_ATTR);
    if (import.meta.env.DEV) {
      const mount_ms = performance.now() - mount_t0;
      mesh_mount_stats.total_ms += mount_ms;
      mesh_mount_stats.count++;
      mesh_mount_stats.max_ms = Math.max(mesh_mount_stats.max_ms, mount_ms);
    }
    if (!(attribute instanceof THREE.BufferAttribute)) return;
    attribute.needsUpdate = true;
    const mounted: MountedLodMesh = { kind: "cube", built, role, attribute };
    registry.meshes.add(mounted);
    return () => void registry.meshes.delete(mounted);
  }, [built, mesh, registry, role]);

  return <primitive object={mesh} />;
});
ChunkMesh.displayName = "ChunkMesh";

// One entry per bundle (matching bundle_assets' own order), each itself one
// entry per that bundle's own variant — build_decor_instances' own return
// shape, once per density level. `secondary` mirrors `LodLayout.secondary`
// exactly (null outside a blend band) rather than decor tracking its own
// independent blend state: decor_build_ref's own per-LodStep cache below
// means a second density level costs one more cache lookup, not a second
// concept, so there's no reason for decor's own blend window to differ from
// cube content's.
type DecorLayoutPair = {
  primary: ModelInstanceTransformList[][];
  secondary: ModelInstanceTransformList[][] | null;
};
const EMPTY_DECOR_PAIR: DecorLayoutPair = { primary: [], secondary: null };

type LodLayout = {
  primary: LodStep;
  secondary: LodStep | null;
  buckets: BuiltChunkBucket[];
  // Populated by decor_build_ref below, entirely independently of `buckets`
  // — see that ref's own doc comment for why the two builds don't share a
  // cache or a rebuild trigger.
  decor: DecorLayoutPair;
  bounds: ChunkBounds | null;
  ready: boolean;
};

// Exported for reference, though Chunk is the only implementation now —
// decor_layer.tsx's own ChunkDecor (a separate, independently-scheduled
// registrant) was retired 2026-09-05 once Chunk absorbed decor's own build
// into its own commit() (see this file's own LodSchedulerProvider doc
// comment and decor_layer.tsx's top doc comment for the fuller account).
export type ChunkLodControl = {
  blend_active: boolean;
  settling: boolean;
  in_frustum: boolean;
  distance2: f64;
  pending: boolean;
  recheck(camera: THREE.Camera, frustum: THREE.Frustum): void;
  commit(): boolean;
};

export type LodSchedulerRegistry = { controls: Set<ChunkLodControl> };
const LodSchedulerContext = createContext<LodSchedulerRegistry | null>(null);

export function use_lod_scheduler(): LodSchedulerRegistry {
  const scheduler = useContext(LodSchedulerContext);
  if (!scheduler) throw new Error("Chunk must be mounted under LodSchedulerProvider");
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

type ChunkProps = { page_idx: IVec; offset: ChunkOffsets; stack: WorldLayerStack; bundle_assets: readonly DecorBundleAssets[] };

const Chunk = memo(({ page_idx, offset, stack, bundle_assets }: ChunkProps) => {
  const low_spec = useSettingsStore((state) => state.low_spec);
  const show_rim_light = useSettingsStore((state) => state.show_rim_light);
  const show_structures = useSettingsStore((state) => state.show_structures);
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
  const decor_revision = useDecorL2ColumnRevision(object_key);

  const bucket_cache_ref = useRef(new Map<LodStep, BuiltChunkBucket>());
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
  // Decor's own build result per LOD step — its own cache, independent of
  // bucket_cache_ref (see decor_build_ref's own doc comment for why the two
  // builds don't share a cache or a rebuild trigger anymore). Keyed by
  // lod_step alone, not by role: 2026-09-06's own blend support (mirroring
  // cube content's primary/secondary pair — see DecorLayoutPair above) needs
  // both steps of a pair built when one is active, and keying by the step
  // itself rather than by which role it currently holds means asking for
  // both is just two lookups against the one cache below, not a second
  // cache or a second build path.
  const decor_cache_ref = useRef(new Map<LodStep, ModelInstanceTransformList[][]>());
  // Cube content only (terrain + solid objects, chunk_mesher.ts's baked
  // mesh) — null return means "a needed scan was just deferred, not built
  // yet", distinct from an empty result (built, genuinely nothing to draw).
  // Decor used to build right alongside this (2026-09-05's "one build step"
  // fix for the terrain/decor ready-state desync), but that meant ANY
  // decor-only change — including set_virtual_hide's hover-driven churn —
  // forced this whole function to re-run, rescanning/rebaking cube content
  // that never actually changed (see objects/store.ts's
  // decor_l2_column_revisions doc comment for the flicker this caused).
  // decor_build_ref below is the split-out replacement; the two still run
  // together whenever cube content itself rebuilds (every call site below
  // calls both), so the original desync fix still holds — decor is never
  // committed without the cube content it sits on having already been built
  // for the exact same step — it's specifically a decor-only change that no
  // longer touches this function at all.
  const build_ref = useRef<(primary: LodStep, secondary: LodStep | null) => BuiltChunkBucket[] | null>(() => []);
  build_ref.current = (primary, secondary) => {
    const steps = secondary === null || secondary === primary ? [primary] : [primary, secondary];
    const missing = steps.filter((step) => !bucket_cache_ref.current.has(step));

    if (missing.length > 0) {
      if (!column_runs_ref.current) {
        let deferred = false;
        const scan_t0 = performance.now();
        const compose_result = with_temporary_composite(stack, key, (volume) => {
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
        // with_temporary_composite itself returns null (its own callback
        // never runs at all) when composite_chunk_layers finds nothing
        // painted by any layer for this exact key — a real, if rare,
        // transient case (a batch of chunks racing their very first build
        // around the same moment can all momentarily read a layer as empty)
        // — NOT the same as "genuinely nothing here, done forever". Falling
        // through to the empty-bucket-list return below would make
        // commit() treat this as a fully successful build (ready: true,
        // pending: false, buckets: []) with no way to ever retry — a
        // permanent black hole exactly where this one-off race happened,
        // and since a whole neighboring batch can hit it at once, a
        // contiguous shape (a hole, a ring) rather than one stray chunk.
        // Returning null here instead — same as a deferred scan — keeps
        // this chunk pending so the scheduler just tries it again.
        if (compose_result === null) return null;
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

  // Decor's own build — a plain per-bundle build_decor_instances call, no
  // volume scan, no dependency on bucket_cache_ref/column_runs_ref at all
  // (ObjectWorldLayer.paint() never paints a voxel for a decor entry, so
  // decor's own output can never depend on anything build_ref above
  // computes). Takes a single LodStep, not a primary/secondary pair — one
  // call per step needed, same as bucket_cache_ref's own per-step entries;
  // build_decor_pair below is what turns "the pair currently active" into
  // "the right one or two calls here". Deliberately NOT keyed off build_ref's
  // own `missing` list: that would re-tie decor's own staleness to cube
  // content's cache, exactly the coupling this split exists to remove (see
  // build_ref's own doc comment).
  const decor_build_ref = useRef<(step: LodStep) => ModelInstanceTransformList[][]>(() => []);
  decor_build_ref.current = (step) => {
    if (!decor_cache_ref.current.has(step)) {
      decor_cache_ref.current.set(step, bundle_assets.map(({ bundle }) => build_decor_instances(tile, step, page_idx, offset.base_x, offset.base_y, bundle)));
    }
    return decor_cache_ref.current.get(step)!;
  };

  // The one place that turns "primary/secondary LodStep pair" into "decor's
  // own built instances for that pair" — every LodLayout.decor assignment
  // below goes through this rather than repeating the null-check/dedup
  // logic build_ref's own `steps` line already established for cube content.
  function build_decor_pair(primary: LodStep, secondary: LodStep | null): DecorLayoutPair {
    return {
      primary: decor_build_ref.current(primary),
      secondary: secondary !== null && secondary !== primary ? decor_build_ref.current(secondary) : null,
    };
  }

  const initial_view = useMemo(
    () => estimate_initial_chunk_view(camera, anchor_registry.cur_map_origin, ivec.add(FROM_L3_IDX(page_idx), page_off)),
    [anchor_registry, camera, page_idx, page_off],
  );

  const [layout, set_layout] = useState<LodLayout>(() => {
    if (!initial_view.in_frustum) {
      return { primary: 64, secondary: null, buckets: [], decor: EMPTY_DECOR_PAIR, bounds: null, ready: false };
    }
    const buckets = build_ref.current(initial_view.lod_step, null);
    if (buckets === null) {
      // Scan deferred on the very first build — start not-ready (same
      // shape as an out-of-frustum chunk) so the first recheck() sets
      // pending and retries, instead of rendering nothing forever.
      return { primary: initial_view.lod_step, secondary: null, buckets: [], decor: EMPTY_DECOR_PAIR, bounds: null, ready: false };
    }
    const initial: LodLayout = {
      primary: initial_view.lod_step,
      secondary: null,
      buckets,
      decor: build_decor_pair(initial_view.lod_step, null),
      bounds: chunk_bounds(buckets),
      ready: true,
    };
    return initial;
  });
  const layout_ref = useRef(layout);
  layout_ref.current = layout;
  const desired_ref = useRef<{ primary: LodStep; secondary: LodStep | null }>({
    primary: initial_view.lod_step,
    secondary: null,
  });
  const source_ref = useRef({ tile, objects_revision, show_structures });

  // A source revision rebuilds the currently displayed LOD, not the entire
  // visible world. Comparing the actual source tuple also prevents React
  // StrictMode's development-only effect replay from rebuilding every chunk.
  // objects_revision here is the SOLID-only revision (see
  // useObjectL2ColumnRevision's own doc comment) — a decor-only change
  // (place/remove/virtual-hide of a tree, say) never touches it, so it never
  // lands here. A solid change still clears/rebuilds decor_cache_ref too,
  // though: a newly placed solid cube can occlude a procedurally-scattered
  // decor candidate that used to render there, which only decor's own build
  // (via get_objects_in_cell) knows how to react to.
  useEffect(() => {
    const previous = source_ref.current;
    if (
      Object.is(previous.tile, tile) && previous.objects_revision === objects_revision &&
      previous.show_structures === show_structures
    ) return;
    source_ref.current = { tile, objects_revision, show_structures };
    const current = layout_ref.current;
    bucket_cache_ref.current.clear();
    decor_cache_ref.current.clear();
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
    const next = { ...current, buckets, decor: build_decor_pair(current.primary, current.secondary), bounds: chunk_bounds(buckets), ready: true };
    layout_ref.current = next;
    set_layout(next);
  }, [tile, objects_revision, show_structures]);

  const decor_source_ref = useRef(decor_revision);

  // decor's own revision-only rebuild — see objects/store.ts's
  // decor_l2_column_revisions doc comment for why this has to be a separate
  // effect from the one above: it must NEVER touch bucket_cache_ref/
  // column_runs_ref (that would reintroduce the exact needless cube
  // rescan/rebake — visible as a flicker — that this split exists to avoid).
  // A decor-only change updates layout.decor alone; buckets/bounds/ready are
  // carried over from whatever the cube-content effect above last set.
  useEffect(() => {
    if (decor_source_ref.current === decor_revision) return;
    decor_source_ref.current = decor_revision;
    const current = layout_ref.current;
    decor_cache_ref.current.clear();
    // Not ready yet (a scan/build is still pending) — the cube-content
    // effect/commit() will populate decor once it actually builds; nothing
    // for this effect to do until then.
    if (!current.ready) return;
    const next = { ...current, decor: build_decor_pair(current.primary, current.secondary) };
    layout_ref.current = next;
    set_layout(next);
  }, [decor_revision]);

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
      // Seeded from the same one-shot camera-relative estimate in_frustum
      // above already used, not a true (Infinity) sentinel — see
      // estimate_initial_chunk_view's own doc comment for why a real,
      // if approximate, distance matters here: the scheduler's recheck loop
      // ranks a same-frame batch of freshly mounted controls by this field,
      // and every one of them starts blend_active with no group mounted yet
      // to measure exactly, so Infinity would make that ranking degrade to
      // registration order for the whole batch.
      distance2: initial_view.distance2,
      pending: false,
      recheck(camera, frustum) {
        const group = group_ref.current;
        if (!group) return;
        const current = layout_ref.current;
        const bounds = current.bounds;
        if (bounds === null) {
          chunk_distance_to_camera(group, CHUNK_WORLD_SPAN * 0.5, CHUNK_WORLD_SPAN * 0.5, 0, camera, center);
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
          for (const step of decor_cache_ref.current.keys()) {
            if (step !== 64) decor_cache_ref.current.delete(step);
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
          decor_cache_ref.current.clear();
          column_runs_ref.current = null;
          // Keep the tiny measured AABB after releasing GPU/surface data so a
          // later polar change can re-enter using real geometry, not z=0.
          const empty: LodLayout = {
            primary: 64,
            secondary: null,
            buckets: [],
            decor: EMPTY_DECOR_PAIR,
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
          decor: build_decor_pair(desired.primary, desired.secondary),
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
          {bucket.cubes.vertex_cnt > 0 && <ChunkMesh built={bucket.cubes} role={bucket.role} low_spec={low_spec} show_rim_light={show_rim_light} registry={registry} />}
        </group>
      ))}
      {
        /* One coordinate convention for everything this chunk renders now:
        build_decor_instances (decor_layer.tsx) returns chunk-local
        positions, matching chunk_mesher.ts's own baked cube positions
        exactly, so decor nests inside this same offset group instead of
        needing its own separate, unoffset one. */
      }
      {bundle_assets.map(({ bundle, geometries, materials }, bi) => {
        const primary_variants = layout.decor.primary[bi];
        const secondary_variants = layout.decor.secondary?.[bi] ?? null;
        if (!primary_variants) return null;
        return (
          <group key={bundle.seed.tag.toString()} name={`chunk-decor/${offset.key}/${bundle.seed.tag.toString()}`}>
            <group name="primary">
              {geometries.map((geometry, variant) => {
                const material = materials[variant];
                const instances = primary_variants[variant];
                return geometry && material && instances && instances.length > 0
                  ? <DecorVariantMesh key={variant} geometry={geometry} material={material} instances={instances} role="primary" registry={registry} />
                  : null;
              })}
            </group>
            {secondary_variants && (
              <group name="secondary">
                {geometries.map((geometry, variant) => {
                  const material = materials[variant];
                  const instances = secondary_variants[variant];
                  return geometry && material && instances && instances.length > 0
                    ? <DecorVariantMesh key={variant} geometry={geometry} material={material} instances={instances} role="secondary" registry={registry} />
                    : null;
                })}
              </group>
            )}
          </group>
        );
      })}
    </group>
  );
});
Chunk.displayName = "Chunk";

type PageChunksProps = { page_idx: IVec; stack: WorldLayerStack; range_anchor: IVec; bundle_assets: readonly DecorBundleAssets[] };

const PageChunks = memo(({ page_idx, stack, range_anchor, bundle_assets }: PageChunksProps) => {
  const page_base = FROM_L3_IDX(page_idx);
  const offsets = useMemo(
    () => in_range_chunks(page_base, range_anchor),
    [page_base, range_anchor],
  );
  return (
    <AnchorNode i={page_base}>
      <group name={`chunk-stage-page/${ivec.repr(page_idx)}`}>
        {offsets.map((offset) => <Chunk key={offset.key} page_idx={page_idx} offset={offset} stack={stack} bundle_assets={bundle_assets} />)}
      </group>
    </AnchorNode>
  );
});
PageChunks.displayName = "PageChunks";

/**
 * Owns the one LodSchedulerRegistry chunk_stage.tsx's own Chunk registers
 * into as a ChunkLodControl — mounted once above ChunkStage (world.tsx).
 * Chunk is the only registrant now: decor used to register its own separate
 * ChunkDecor per (chunk, decor kind) here too, competing for this same
 * budget as an independent component with its own `ready` state; as of
 * 2026-09-05 Chunk builds decor alongside its own cube content in one
 * commit() instead (see decor_layer.tsx's own top doc comment), so this
 * registry now only ever holds one control per chunk position, not up to
 * five. Everything here — the registry, the frustum, the recheck/commit
 * loop, the dev stats — used to live inside ChunkStage itself, back when it
 * was the only registrant; none of it is terrain-specific, so it moved out
 * rather than ChunkStage keeping ownership of something it no longer
 * exclusively uses.
 */
export function LodSchedulerProvider({ children }: { children: ReactNode }) {
  const scheduler = useMemo<LodSchedulerRegistry>(() => ({ controls: new Set(), ready: new Set() }), []);
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
    update_frustum(frustum, state.camera, projection);

    const recheck_t0 = import.meta.env.DEV ? performance.now() : 0;
    // Same cooperative shape as the commit loop below: the first eligible
    // control this frame always runs (forward progress guaranteed even on
    // a slow frame), the rest are time-boxed. Profiled: during a fast zoom,
    // settling_lod/blend_active can both spike into the dozens at once, and
    // recheck()-ing every one of them every single frame (previously
    // unconditional) cost up to ~23ms by itself — a stale blend/settling
    // flag for one skipped frame is a much smaller visual cost than that.
    //
    // Sorted, not left in whatever order controls happened to register into
    // the Set — plain registration order has no relationship to real 3D
    // distance from the camera, so under real budget pressure (a big
    // pan/zoom mounting many chunks in one frame) it could let a handful of
    // far chunks resolve first purely because they mounted first, while much
    // closer chunks queued behind them for a frame or more — the opposite
    // of the "resolves outward from the camera" this whole scheduler exists
    // for. Same (in_frustum desc, distance2 asc) key the commit loop below
    // sorts pending by — one shared notion of priority, not two.
    //
    // distance2 is always a real, if sometimes approximate, camera-relative
    // value now (every control seeds it from estimate_initial_chunk_view at
    // construction — see that control's own distance2 field comment — never
    // a true Infinity sentinel), so this alone is enough: a brand new
    // control mounted this very frame competes fairly on real distance
    // against an older still-pending one instead of needing a separate
    // "urgent" tier. An earlier version of this sort DID give blend_active/
    // settling controls a whole tier ahead of distance — that seemed right
    // (their own first-ever recheck matters most) but backfires under
    // sustained camera movement: a continuously panning camera keeps
    // minting brand-new blend_active chunks at its leading edge, an
    // unbounded stream that could occupy the "urgent" tier forever and
    // starve an older, still-pending, genuinely-closer chunk that merely
    // isn't blend_active/settling anymore — exactly "far keeps cutting in
    // line over near" during continuous motion, not just a one-off frame.
    // Distance alone doesn't have that failure mode: a far new chunk sorts
    // behind a near old one no matter how continuously the camera moves.
    //
    // Filtered to this frame's actually-eligible controls BEFORE sorting,
    // not sorted-then-filtered — controls can number in the thousands
    // (every terrain chunk across LOAD_RADIUS's whole page window, times
    // every decor kind on top), and on all but a full_recheck frame only
    // the blend_active/settling subset (usually far smaller) needs
    // anything done at all. Sorting the full set every single frame
    // regardless of eligibility would size this copy+sort to the total
    // registrant count instead of the actual per-frame workload — real,
    // needless GC pressure and comparisons on the common (non-full_recheck)
    // frame, for a subset most of which was about to be skipped anyway.
    const eligible = controls.filter((control) => full_recheck || control.blend_active || control.settling);
    eligible.sort((a, b) => Number(b.in_frustum) - Number(a.in_frustum) || a.distance2 - b.distance2);
    const recheck_deadline = performance.now() + LOD_WORK_BUDGET_MS;
    let blend_active = false;
    let rechecked: u32 = 0;
    for (const control of eligible) {
      if (rechecked === 0 || performance.now() < recheck_deadline) {
        control.recheck(state.camera, frustum);
        rechecked++;
      }
      // Every blend_active-true control is by construction included in
      // eligible above (it's one of the three needs_recheck conditions),
      // so this OR over just the eligible subset still sees every one of
      // them — equivalent to scanning all of controls for this specific
      // flag, just without paying to visit the ones that couldn't have it.
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

  return <LodSchedulerContext.Provider value={scheduler}>{children}</LodSchedulerContext.Provider>;
}

export type ChunkStageProps = { stack: WorldLayerStack; decor_bundles: readonly DecorKindBundle[] };

/** Renders terrain/objects AND every decor kind together — see this file's
 * own top doc comment and decor_layer.tsx's for why decor no longer has its
 * own top-level component (DecorLayer, retired 2026-09-05). `decor_bundles`
 * should be a stable (module-level) reference — see world.tsx's own
 * DECOR_BUNDLES — useAllDecorBundleAssets relies on that to avoid
 * needlessly re-slicing its own already-loaded assets every render. */
export const ChunkStage = memo(({ stack, decor_bundles }: ChunkStageProps) => {
  const map_center = useMapStore((state) => state.map_center);
  const tile_map = useTileStore((state) => state.tile_map);
  const bundle_assets = useAllDecorBundleAssets(decor_bundles);

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
    <group name="chunk-stage">
      {visible_pages.map((page_idx) => (
        <PageChunks key={page_idx.toString()} page_idx={page_idx} stack={stack} range_anchor={range_anchor} bundle_assets={bundle_assets} />
      ))}
    </group>
  );
});
ChunkStage.displayName = "ChunkStage";

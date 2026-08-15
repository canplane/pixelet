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

import { useDebugStore } from "@/app/dev/debug_store.ts";
import { useMapStore } from "@/modules/map/store.ts";
import { light_cfg } from "@/modules/world/environment/config.ts";
import { useAnchorRegistry } from "@/modules/world/anchor/context.tsx";
import { AnchorNode } from "@/modules/world/anchor/node.tsx";
import { l2_column_key_of, useObjectL2ColumnRevision } from "@/modules/world/objects/store.ts";
import { RENDER_RADIUS } from "@/modules/world/terrain/config.ts";
import { useTileStore } from "@/modules/world/terrain/store.ts";
import { CHEBYSHEV_DIST } from "@/modules/world/terrain/use_tile_fetcher.ts";
import { NO_LAYER } from "@/render/layer/pipeline/volume.ts";
import { in_range_chunks } from "@/render/chunk_range.ts";
import { composite_chunk_surface_layers, compositor_cache_stats, with_temporary_composite } from "@/render/layer/pipeline/compositor.ts";
import { GROUND_LAYERS_SIDE, GROUND_LAYERS_TOP } from "@/render/texture_array.ts";
import { build_chunk_surface, type ChunkSurface } from "@/render/layer/pipeline/chunk_mesher.ts";
import {
  type ChunkLodRole,
  build_chunk_bucket,
  build_chunk_bucket_from_surface,
  type BuiltChunkBucket,
  type BuiltChunkMesh,
  chunk_bounds,
  type ChunkBounds,
  type ChunkOffsets,
} from "@/render/layer/pipeline/chunk_bucket.ts";
import type { ChunkKey, WorldLayerStack } from "@/render/layer/pipeline/contract.ts";
import { get_lod_guide, get_lod_pair_visibility, type LODStep, step_lod_toward } from "@/render/lod.ts";

const LOD_RECHECK_INTERVAL: f64 = 0.15;
const LOD_WORK_BUDGET_MS: f64 = 4;
const LOD_MAX_CHUNKS_PER_FRAME: u32 = 8;
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
}): void {
  shader.uniforms.u_ground_layers_top = { value: GROUND_LAYERS_TOP };
  shader.uniforms.u_ground_layers_side = { value: GROUND_LAYERS_SIDE };
  shader.uniforms.u_low_spec = { value: true };
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
      varying float v_is_side;`,
    )
    .replace(
      "void main() {",
      `void main() {
        v_${INSTANCE_RGB_ATTR} = ${INSTANCE_RGB_ATTR};
        v_${INSTANCE_LAYER_TAPS_ATTR} = ${INSTANCE_LAYER_TAPS_ATTR};
        v_${INSTANCE_VISIBLE_ATTR} = ${INSTANCE_VISIBLE_ATTR};
        v_box_uv = uv;
        v_is_side = (abs(normal.z) < 0.5) ? 1.0 : 0.0;`,
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
      uniform sampler2DArray u_ground_layers_top;
      uniform sampler2DArray u_ground_layers_side;
      uniform bool u_low_spec;
      vec3 sample_layer_tap(float layer_idx, vec3 fallback_rgb) {
        if (layer_idx > ${NO_LAYER}.0 - 0.5) return fallback_rgb;
        return v_is_side > 0.5
          ? texture(u_ground_layers_side, vec3(v_box_uv, layer_idx)).rgb
          : texture(u_ground_layers_top, vec3(v_box_uv, layer_idx)).rgb;
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
      vec4 diffuseColor = vec4(diffuse * base_rgb, opacity);`,
    )
    .replace("vec3 totalEmissiveRadiance = emissive;", `vec3 totalEmissiveRadiance = base_rgb * ${light_cfg.FLOOR};`);
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

function create_material(): THREE.MeshStandardMaterial {
  const material = new THREE.MeshStandardMaterial({ roughness: 1, metalness: 0 });
  material.onBeforeCompile = (shader) => {
    inject_instance_shader(shader);
    shader.uniforms.u_low_spec.value = useDebugStore.getState().low_spec;
    material.userData.shader = shader;
  };
  return material;
}

type ChunkInstanceMeshProps = {
  built: BuiltChunkMesh;
  role: ChunkLodRole;
  base: THREE.BoxGeometry;
  low_spec: boolean;
  registry: ChunkMeshRegistry;
};

const ChunkInstanceMesh = memo(({ built, role, base, low_spec, registry }: ChunkInstanceMeshProps) => {
  const material = useMemo(create_material, []);
  const mesh = useMemo(
    () => new THREE.InstancedMesh(base.clone() as THREE.BufferGeometry, material, 1),
    [base, material],
  );

  useEffect(() => () => {
    mesh.geometry.dispose();
    material.dispose();
  }, [material, mesh]);
  useEffect(() => {
    const shader = material.userData.shader as { uniforms: Record<string, THREE.IUniform> } | undefined;
    if (shader) shader.uniforms.u_low_spec.value = low_spec;
  }, [material, low_spec]);

  useLayoutEffect(() => {
    const previous_geometry = mesh.geometry;
    const geometry = create_geometry(built, base);
    mesh.geometry = geometry;
    previous_geometry.dispose();
    mesh.instanceMatrix = new THREE.InstancedBufferAttribute(built.matrices, 16);
    mesh.instanceMatrix.needsUpdate = true;
    mesh.count = built.cnt;
    mesh.visible = built.cnt > 0;
    built.visibility.fill(role === "primary" ? 1 : 0);
    mesh.computeBoundingBox();
    mesh.computeBoundingSphere();
    const attribute = geometry.getAttribute(INSTANCE_VISIBLE_ATTR);
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
  surface_cached: boolean;
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
  const low_spec = useDebugStore((state) => state.low_spec);
  const show_structures = useDebugStore((state) => state.show_structures);
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

  const surface_ref = useRef<ChunkSurface | null>(null);
  const bucket_cache_ref = useRef(new Map<LODStep, BuiltChunkBucket>());
  const build_ref = useRef<(primary: LODStep, secondary: LODStep | null) => BuiltChunkBucket[]>(() => []);
  build_ref.current = (primary, secondary) => {
    const steps = secondary === null || secondary === primary ? [primary] : [primary, secondary];
    const missing = steps.filter((step) => !bucket_cache_ref.current.has(step));

    if (missing.length > 0 && surface_ref.current !== null && missing.every((step) => step > 1)) {
      for (const step of missing) {
        bucket_cache_ref.current.set(
          step,
          build_chunk_bucket_from_surface(surface_ref.current, step, "primary"),
        );
      }
    } else if (missing.length > 0 && missing.every((step) => step > 1)) {
      const surface = composite_chunk_surface_layers(stack, key);
      if (surface !== undefined && surface !== null) {
        surface_ref.current = surface;
        for (const step of missing) {
          bucket_cache_ref.current.set(
            step,
            build_chunk_bucket_from_surface(surface, step, "primary"),
          );
        }
      } else if (surface === undefined) {
        with_temporary_composite(stack, key, (volume) => {
          const fallback_surface = build_chunk_surface(volume);
          surface_ref.current = fallback_surface;
          for (const step of missing) {
            bucket_cache_ref.current.set(
              step,
              build_chunk_bucket(volume, step, "primary", fallback_surface),
            );
          }
        });
      }
    } else if (missing.length > 0) {
      with_temporary_composite(stack, key, (volume) => {
        const surface = surface_ref.current ?? build_chunk_surface(volume);
        surface_ref.current = surface;
        for (const step of missing) {
          bucket_cache_ref.current.set(
            step,
            build_chunk_bucket(volume, step, "primary", surface),
          );
        }
      });
    }

    const keep = new Set(steps);
    for (const step of bucket_cache_ref.current.keys()) {
      if (!keep.has(step)) bucket_cache_ref.current.delete(step);
    }
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
    surface_ref.current = null;
    bucket_cache_ref.current.clear();
    if (!current.ready) {
      if (current.bounds !== null) {
        const invalidated = { ...current, bounds: null };
        layout_ref.current = invalidated;
        set_layout(invalidated);
      }
      return;
    }
    const buckets = build_ref.current(current.primary, current.secondary);
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
      surface_cached: surface_ref.current !== null,
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
          surface_ref.current = null;
          this.surface_cached = false;
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
        this.surface_cached = surface_ref.current !== null;

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
          surface_ref.current = null;
          bucket_cache_ref.current.clear();
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
          this.surface_cached = false;
          return true;
        }
        const desired = desired_ref.current;
        const buckets = build_ref.current(desired.primary, desired.secondary);
        const next: LodLayout = {
          primary: desired.primary,
          secondary: desired.secondary,
          buckets,
          bounds: chunk_bounds(buckets),
          ready: true,
        };
        layout_ref.current = next;
        set_layout(next);
        this.pending = false;
        this.surface_cached = surface_ref.current !== null;
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
          {bucket.terrain.cnt > 0 && <ChunkInstanceMesh built={bucket.terrain} role={bucket.role} base={TERRAIN_BOX} low_spec={low_spec} registry={registry} />}
          {bucket.objects.cnt > 0 && <ChunkInstanceMesh built={bucket.objects} role={bucket.role} base={OBJECT_BOX} low_spec={low_spec} registry={registry} />}
        </group>
      ))}
    </group>
  );
});
Chunk.displayName = "Chunk";

type PageChunksProps = { page_idx: IVec; stack: WorldLayerStack; range_anchor: IVec };

const PageChunks = memo(({ page_idx, stack, range_anchor }: PageChunksProps) => {
  const page_base = FROM_L3_IDX(page_idx);
  const offsets = useMemo(
    () => in_range_chunks(page_base, range_anchor),
    [page_base, range_anchor],
  );
  return (
    <AnchorNode i={page_base}>
      <group name={`chunk-stage-page/${ivec.repr(page_idx)}`}>
        {offsets.map((offset) => <Chunk key={offset.key} page_idx={page_idx} offset={offset} stack={stack} />)}
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
  const projection = useMemo(() => new THREE.Matrix4(), []);

  useFrame((state, delta) => {
    const controls = Array.from(scheduler.controls);
    if (controls.length === 0) return;
    lod_elapsed.current += delta;
    const full_recheck = lod_elapsed.current >= LOD_RECHECK_INTERVAL;
    if (full_recheck) lod_elapsed.current = 0;
    projection.multiplyMatrices(state.camera.projectionMatrix, state.camera.matrixWorldInverse);
    frustum.setFromProjectionMatrix(projection);

    let blend_active = false;
    for (const control of controls) {
      if (full_recheck || control.blend_active || control.settling) control.recheck(state.camera, frustum);
      blend_active ||= control.blend_active;
    }

    const pending_total = controls.filter((control) => control.pending);
    // Visible builds/refinements win. Pending work outside the frustum only
    // drops a formerly visible chunk's derived surface and GPU buffers.
    const pending = pending_total.sort((a, b) => Number(b.in_frustum) - Number(a.in_frustum) || a.distance2 - b.distance2);
    const deadline = performance.now() + LOD_WORK_BUDGET_MS;
    let committed: u32 = 0;
    for (const control of pending) {
      if (committed >= LOD_MAX_CHUNKS_PER_FRAME || (committed > 0 && performance.now() >= deadline)) break;
      if (control.commit()) committed++;
    }
    if (import.meta.env.DEV) {
      const debug_stats = {
        chunks: controls.length,
        in_frustum: controls.filter((control) => control.in_frustum).length,
        pending_lod: pending.length - committed,
        pending_lod_total: pending_total.length - committed,
        blend_active: controls.filter((control) => control.blend_active).length,
        settling_lod: controls.filter((control) => control.settling).length,
        surface_cached: controls.filter((control) => control.surface_cached).length,
        draw_calls: state.gl.info.render.calls,
        triangles: state.gl.info.render.triangles,
        geometries: state.gl.info.memory.geometries,
        textures: state.gl.info.memory.textures,
        compositor: compositor_cache_stats(),
      };
      (globalThis as typeof globalThis & { __PIXELET_RENDER_STATS__?: unknown }).__PIXELET_RENDER_STATS__ = debug_stats;
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

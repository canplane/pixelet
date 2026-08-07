// @pxl/web/modules/world/terrain/chunk.tsx

import { memo, useEffect, useLayoutEffect, useMemo, useRef } from "react";
import * as THREE from "three";
import { useFrame } from "@react-three/fiber";

import type { f64, u32 } from "@xpute/core/kit/word.ts";

import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L2_CENTER } from "@pxl/shared/spec/space/indexer.ts";

import { GROUND_CH_KEYS, type GroundChannelKey, type Terrain } from "@pxl/shared/bake/format.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";
import { get_instance_cnt, get_lod_guide, type LODStep } from "@/render/lod.ts";
import { useDebugStore } from "@/app/dev/debug_store.ts";

import { create_heightmap, create_splatmap } from "./shader/tex.ts";
import { create_mat } from "./shader/mat.ts";

const ROW_STRIDE: u32 = CHUNK_CELL_SPAN;
const CHUNK_CELL_SPAN2: u32 = CHUNK_CELL_SPAN * CHUNK_CELL_SPAN;

const CHUNK_BOX = new THREE.BoxGeometry(1, 1, 1);

const GROUND_Z_BIAS: f64 = -0.01;

/** Seconds between LOD distance re-checks (per chunk). */
const LOD_RECHECK_INTERVAL: f64 = 0.15;

type TerrainChunkProps = {
  /** Page-relative local chunk offset. */
  page_off: IVec;
  chunk: Terrain;
};
export const TerrainChunk = memo((props: TerrainChunkProps) => {
  const { page_off, chunk } = props;
  const { ground, ele } = chunk;

  const mesh_ref = useRef<THREE.InstancedMesh>(null);
  // second draw for the LOD-boundary blend (see lod.ts's get_lod_guide and
  // shader.ts's position_core) — always mounted, but left at count=0 outside
  // a blend band so it costs nothing when this chunk isn't transitioning.
  // kept as a permanent second mesh rather than conditionally mounted:
  // get_lod_guide's blend_active flips in and out multiple times a second
  // right at a band's edge, and mounting/unmounting a whole instancedMesh +
  // material on every flip (React commit, GPU buffer alloc/dispose) would
  // cost far more than one always-present zero-count draw does.
  const blend_mesh_ref = useRef<THREE.InstancedMesh>(null);

  const splatmaps = useMemo(() => {
    return Object.fromEntries(
      GROUND_CH_KEYS.map((key) => [key, create_splatmap(ground[key], ROW_STRIDE)]),
    ) as Record<GroundChannelKey, THREE.DataTexture>;
  }, [ground]);
  const heightmap = useMemo(() => create_heightmap(ele, ROW_STRIDE), [ele]);
  const { mat, depth_mat } = useMemo(() => create_mat(splatmaps, heightmap, ROW_STRIDE, U_PER_CELL), [splatmaps, heightmap]);
  // independent material instance (not the same object as mat/depth_mat) —
  // it needs its own u_lod_step/u_blend_role, which differ from the
  // primary's whenever a blend is active. Cheap to keep around otherwise:
  // same source textures, no extra GPU memory beyond the small uniform
  // buffers and a second (usually 0-instance) draw call.
  const { mat: blend_mat, depth_mat: blend_depth_mat } = useMemo(
    () => create_mat(splatmaps, heightmap, ROW_STRIDE, U_PER_CELL),
    [splatmaps, heightmap],
  );

  useEffect(() => {
    return () => {
      Object.values(splatmaps).forEach((tex) => tex.dispose());
      heightmap.dispose();
      mat.dispose();
      depth_mat.dispose();
      blend_mat.dispose();
      blend_depth_mat.dispose();
    };
  }, [splatmaps, heightmap, mat, depth_mat, blend_mat, blend_depth_mat]);

  // dev_panel.tsx's "건물" toggle. A live uniform write, not a useMemo
  // dep on mat/blend_mat above — those two are already the expensive-to-
  // rebuild ShaderMaterial instances (real GPU program compiles), and this
  // is a debug button that can flip on every currently-loaded chunk at
  // once. depth_mat/blend_depth_mat don't need their own write: mat.ts's
  // core_uniforms object is shared by reference between mat/depth_mat (and
  // separately between blend_mat/blend_depth_mat), the same trick
  // u_lod_step/u_blend_t already rely on there.
  const show_structures = useDebugStore((state) => state.show_structures);
  useEffect(() => {
    mat.uniforms.u_show_structures.value = show_structures;
    blend_mat.uniforms.u_show_structures.value = show_structures;
  }, [show_structures, mat, blend_mat]);

  const chunk_world_pos = useRef(new THREE.Vector3());

  useLayoutEffect(() => {
    if (!mesh_ref.current) return;
    mesh_ref.current.frustumCulled = false;
    mesh_ref.current.count = CHUNK_CELL_SPAN2;
    // R3F has no `customDepthMaterial` JSX prop — it's an imperative-only
    // three.js field WebGLShadowMap reads directly off the object, so it has
    // to be set by hand here rather than declared on <instancedMesh>.
    mesh_ref.current.customDepthMaterial = depth_mat;
  }, [depth_mat]);

  useLayoutEffect(() => {
    if (!blend_mesh_ref.current) return;
    blend_mesh_ref.current.frustumCulled = false;
    blend_mesh_ref.current.count = 0;
    blend_mesh_ref.current.customDepthMaterial = blend_depth_mat;
  }, [blend_depth_mat]);

  // ---- LOD ----

  const current_lod_step = useRef<LODStep>(1);
  const lod_recheck_elapsed = useRef<f64>(0);
  useFrame(({ camera }, delta) => {
    if (!mesh_ref.current || !blend_mesh_ref.current) return;

    // updateWorldMatrix + getWorldPosition walks this chunk's whole parent chain, and
    // with up to a few hundred chunks loaded at once that's a real per-frame cost.
    // LOD doesn't need per-frame precision, so only re-check a handful of times a
    // second instead of on every tick.
    lod_recheck_elapsed.current += delta;
    if (lod_recheck_elapsed.current < LOD_RECHECK_INTERVAL) return;
    lod_recheck_elapsed.current = 0;

    // R3F/three.js only refreshes matrixWorld once per tick, after all useFrame
    // callbacks run — reading it here would be one frame stale. That's usually
    // negligible, but right after a floating-origin rebase (an instant, large
    // position snap) it's a real error for that one frame, which can push the
    // LOD up and get stuck there due to the hysteresis below. Force a fresh
    // parent-chain update before sampling.
    mesh_ref.current.updateWorldMatrix(true, false);
    mesh_ref.current.getWorldPosition(chunk_world_pos.current);

    // horizontal (X/Y, scene is Z-up) distance from the camera to the chunk center.
    // terrain height is a GPU-only vertex-shader displacement (heightmap texture) —
    // the chunk's own object position is always at Z≈0, so a 3D distance here would
    // pick up the camera's real-world elevation (map_center.Z, via the floating-origin
    // frame translation) as if the chunk had moved away, coarsening LOD the higher up
    // you are even though the on-screen distance to the ground hasn't changed.
    const dx = camera.position.x - chunk_world_pos.current.x;
    const dy = camera.position.y - chunk_world_pos.current.y;
    const dist = Math.hypot(dx, dy);

    // derive the next LOD state from distance.
    const guide = get_lod_guide(dist, current_lod_step.current);

    // apply GPU-side changes only when the LOD step actually changes.
    if (guide.lod_step !== current_lod_step.current) {
      current_lod_step.current = guide.lod_step;
      mesh_ref.current.count = guide.instance_cnt;
      mat.uniforms.u_lod_step.value = guide.lod_step;
      depth_mat.uniforms.u_lod_step.value = guide.lod_step;
    }

    if (guide.blend_active && guide.blend_finer_step !== null && guide.blend_coarser_step !== null) {
      const blend_step: LODStep = guide.lod_step === guide.blend_finer_step ? guide.blend_coarser_step : guide.blend_finer_step;
      const primary_role: number = guide.lod_step === guide.blend_finer_step ? 0 : 1;

      mat.uniforms.u_blend_t.value = guide.coarse_fraction;
      mat.uniforms.u_blend_role.value = primary_role;
      depth_mat.uniforms.u_blend_t.value = guide.coarse_fraction;
      depth_mat.uniforms.u_blend_role.value = primary_role;

      if (blend_mesh_ref.current.count === 0 || blend_mat.uniforms.u_lod_step.value !== blend_step) {
        blend_mesh_ref.current.count = get_instance_cnt(blend_step);
        blend_mat.uniforms.u_lod_step.value = blend_step;
        blend_depth_mat.uniforms.u_lod_step.value = blend_step;
      }
      blend_mat.uniforms.u_blend_t.value = guide.coarse_fraction;
      blend_mat.uniforms.u_blend_role.value = 1 - primary_role;
      blend_depth_mat.uniforms.u_blend_t.value = guide.coarse_fraction;
      blend_depth_mat.uniforms.u_blend_role.value = 1 - primary_role;
    } else if (blend_mesh_ref.current.count !== 0) {
      // left the band — collapse back to a single normal draw. Resetting
      // u_blend_t/role on the primary isn't strictly required (role 0 + t=0
      // is the inert default it already had outside a band) but cheap
      // insurance against a stale value if this ever gets hit mid-band by a
      // sudden jump (see the hysteresis-mismatch guard in lod.ts).
      mat.uniforms.u_blend_t.value = 0;
      mat.uniforms.u_blend_role.value = 0;
      depth_mat.uniforms.u_blend_t.value = 0;
      depth_mat.uniforms.u_blend_role.value = 0;
      blend_mesh_ref.current.count = 0;
    }
  });

  const chunk_center: IVec = L2_CENTER(page_off);
  const chunk_position: [number, number, number] = [
    ivec.to_real(ivec.X(chunk_center)),
    ivec.to_real(ivec.Y(chunk_center)),
    GROUND_Z_BIAS,
  ];
  return (
    <>
      <instancedMesh
        ref={mesh_ref}
        // the third arg is the maximum allocation count for the densest LOD.
        args={[CHUNK_BOX, undefined, CHUNK_CELL_SPAN2]}
        position={chunk_position}
        frustumCulled={false}
        // castShadow: this terrain *is* where "buildings" live (elevated
        // cells, no separate 3D building mesh exists yet) — without this,
        // nothing in real content casts a shadow at all. Backed by
        // customDepthMaterial above so the caster's silhouette matches the
        // heightmap-displaced shape instead of three's default (flat,
        // undisplaced) depth material.
        castShadow
        receiveShadow
      >
        <primitive object={mat} attach="material" />
      </instancedMesh>

      <instancedMesh
        ref={blend_mesh_ref}
        args={[CHUNK_BOX, undefined, CHUNK_CELL_SPAN2]}
        position={chunk_position}
        frustumCulled={false}
        castShadow
        receiveShadow
      >
        <primitive object={blend_mat} attach="material" />
      </instancedMesh>
    </>
  );
});
TerrainChunk.displayName = "TerrainChunk";

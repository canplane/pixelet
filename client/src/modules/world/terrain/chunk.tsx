// @pxl/web/modules/world/terrain/chunk.tsx

import { memo, useEffect, useLayoutEffect, useMemo, useRef } from "react";
import * as THREE from "three";
import { useFrame, useThree } from "@react-three/fiber";

import type { f64, u32 } from "@pixelet/shared/kit/word.ts";

import { type IVec, ivec } from "@pixelet/shared/spec/space/pvf3/mod.ts";
import { L2_CENTER } from "@pixelet/shared/spec/space/indexer.ts";

import { GROUND_CH_KEYS, type GroundChannelKey, type Terrain } from "@pixelet/shared/bake/format.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pixelet/shared/bake/constants.ts";
import { get_lod_guide, type LODStep } from "@/render/lod.ts";

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

  const { gl } = useThree();
  const mesh_ref = useRef<THREE.InstancedMesh>(null);

  const splatmaps = useMemo(() => {
    return Object.fromEntries(
      GROUND_CH_KEYS.map((key) => [key, create_splatmap(ground[key], ROW_STRIDE, gl as THREE.WebGLRenderer)]),
    ) as Record<GroundChannelKey, THREE.DataTexture>;
  }, [gl, ground]);
  const heightmap = useMemo(() => create_heightmap(ele, ROW_STRIDE), [ele]);
  const mat = useMemo(() => create_mat(splatmaps, heightmap, ROW_STRIDE, U_PER_CELL), [splatmaps, heightmap]);

  useEffect(() => {
    return () => {
      Object.values(splatmaps).forEach((tex) => tex.dispose());
      heightmap.dispose();
      mat.dispose();
    };
  }, [splatmaps, heightmap, mat]);

  const chunk_world_pos = useRef(new THREE.Vector3());

  useLayoutEffect(() => {
    if (!mesh_ref.current) return;
    mesh_ref.current.frustumCulled = false;
    mesh_ref.current.count = CHUNK_CELL_SPAN2;
  }, []);

  // ---- LOD ----

  const current_lod_step = useRef<LODStep>(1);
  const lod_recheck_elapsed = useRef<f64>(0);
  useFrame(({ camera }, delta) => {
    if (!mesh_ref.current) return;

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

    // Horizontal (X/Y, scene is Z-up) distance from the camera to the chunk center.
    // Terrain height is a GPU-only vertex-shader displacement (heightmap texture) —
    // the chunk's own object position is always at Z≈0, so a 3D distance here would
    // pick up the camera's real-world elevation (map_center.Z, via the floating-origin
    // frame translation) as if the chunk had moved away, coarsening LOD the higher up
    // you are even though the on-screen distance to the ground hasn't changed.
    const dx = camera.position.x - chunk_world_pos.current.x;
    const dy = camera.position.y - chunk_world_pos.current.y;
    const dist = Math.hypot(dx, dy);

    // Derive the next LOD state from distance.
    const { lod_step, instance_cnt } = get_lod_guide(dist, current_lod_step.current);

    // Apply GPU-side changes only when the LOD step actually changes.
    if (lod_step !== current_lod_step.current) {
      current_lod_step.current = lod_step;
      mesh_ref.current.count = instance_cnt;
      mat.uniforms.u_lod_step.value = lod_step;
    }
  });

  const chunk_center: IVec = L2_CENTER(page_off);
  return (
    <instancedMesh
      ref={mesh_ref}
      // The third arg is the maximum allocation count for the densest LOD.
      args={[CHUNK_BOX, undefined, CHUNK_CELL_SPAN2]}
      position={[
        ivec.to_real(ivec.X(chunk_center)),
        ivec.to_real(ivec.Y(chunk_center)),
        GROUND_Z_BIAS,
      ]}
      frustumCulled={false}
    >
      <primitive object={mat} attach="material" />
    </instancedMesh>
  );
});
TerrainChunk.displayName = "TerrainChunk";

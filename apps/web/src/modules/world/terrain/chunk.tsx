// @pxl/web/modules/world/terrain/chunk.tsx

import { memo, useEffect, useLayoutEffect, useMemo, useRef } from "react";
import * as THREE from "three";
import { useFrame, useThree } from "@react-three/fiber";

import type { f64, u32 } from "@xpute/spec/lib/kit/word.ts";

import { type IVec, ivec } from "@pxl/spec/space/pvf3/mod.ts";
import { L2_CENTER } from "@pxl/spec/space/indexer.ts";

import { GROUND_CH_KEYS, type GroundChannelKey, type Terrain } from "@pxl/bake/terrain/format.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/bake/terrain/constants.ts";
import { get_lod_guide, type LODStep } from "@pxl/render/lod.ts";

import { create_heightmap, create_splatmap } from "./shader/tex.ts";
import { create_mat } from "./shader/mat.ts";

const ROW_STRIDE: u32 = CHUNK_CELL_SPAN;
const CHUNK_CELL_SPAN2: u32 = CHUNK_CELL_SPAN * CHUNK_CELL_SPAN;

const CHUNK_BOX = new THREE.BoxGeometry(1, 1, 1);

const GROUND_Z_BIAS: f64 = -0.01;

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
  useFrame(({ camera }) => {
    if (!mesh_ref.current) return;

    mesh_ref.current.getWorldPosition(chunk_world_pos.current);

    // Physical distance from the camera to the chunk center.
    const dist = camera.position.distanceTo(chunk_world_pos.current);

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

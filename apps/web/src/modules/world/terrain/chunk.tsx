// @pxl/web/modules/world/terrain/chunk.tsx

import React, { useEffect, useLayoutEffect, useMemo, useRef } from "react";
import * as THREE from "three";
import { useFrame, useThree } from "@react-three/fiber";

import type { f64, u32 } from "@xpute/spec/lib/kit/word.ts";

import { type IVec, ivec } from "@pxl/spec/space/pvf3/mod.ts";
import { L2_CENTER } from "@pxl/spec/space/indexer.ts";
import { to3 } from "@pxl/spec/space/adapter/three.ts";

import { GROUND_CH_KEYS, type GroundChannelKey, type Terrain } from "@pxl/bake/terrain/format.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/bake/terrain/constants.ts";
import { get_lod_guide, type LODStep } from "@pxl/render/lod.ts";

import { create_heightmap, create_splatmap } from "./shader/tex.ts";
import { create_mat } from "./shader/mat.ts";

import { render_cfg } from "@/modules/world/render/config.ts";

const { FLOOR_ROTATION } = render_cfg.layout;

const ROW_STRIDE: u32 = CHUNK_CELL_SPAN;
const CHUNK_CELL_SPAN2: u32 = CHUNK_CELL_SPAN * CHUNK_CELL_SPAN;

const BOX = new THREE.BoxGeometry(1, 1, 1);

const GROUND_Z_BIAS: f64 = -0.01;

type TerrainChunkProps = {
  // page-relative local offset
  page_off: IVec;
  chunk: Terrain;
};
export const TerrainChunk = React.memo((props: TerrainChunkProps) => {
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

  const world_pos = useRef(new THREE.Vector3());

  useLayoutEffect(() => {
    if (!mesh_ref.current) return;
    mesh_ref.current.frustumCulled = false;
    mesh_ref.current.count = CHUNK_CELL_SPAN2;
  }, []);

  // ---- LOD ----

  const current_lod_step = useRef<LODStep>(1);
  useFrame(({ camera }) => {
    if (!mesh_ref.current) return;

    mesh_ref.current.getWorldPosition(world_pos.current);

    // 1. 카메라와 청크 중심점 간의 실제 물리적 거리 계산
    const dist = camera.position.distanceTo(world_pos.current);

    // 2. 거리 기반으로 LOD Step 및 인스턴스 렌더링 개수 계산
    const { lod_step, instance_cnt } = get_lod_guide(dist, current_lod_step.current);

    // 3. LOD가 변경되었을 때만 GPU 레벨 업데이트 (Zero Re-render!)
    if (lod_step !== current_lod_step.current) {
      current_lod_step.current = lod_step;

      // 인스턴스 렌더링 개수 조절 (Geometry 밀도 변화)
      mesh_ref.current.count = instance_cnt;

      // 쉐이더 내에서 텍스처 샘플링 간격을 조절하는 Uniform 업데이트
      mat.uniforms.u_lod_step.value = lod_step;
    }
  });

  const chunk_center: IVec = L2_CENTER(page_off);
  return (
    <instancedMesh
      ref={mesh_ref}
      // args의 세 번째 인자는 '최대 할당 개수'이므로 L1 해상도(가장 촘촘한 상태)의 개수를 미리 잡아둡니다.
      args={[BOX, undefined, CHUNK_CELL_SPAN2]}
      position={to3([
        ivec.to_real(ivec.X(chunk_center)),
        ivec.to_real(ivec.Y(chunk_center)),
        GROUND_Z_BIAS,
      ])}
      rotation={FLOOR_ROTATION as unknown as THREE.Euler}
      frustumCulled={false}
    >
      <primitive object={mat} attach="material" />
    </instancedMesh>
  );
});
TerrainChunk.displayName = "TerrainChunk";

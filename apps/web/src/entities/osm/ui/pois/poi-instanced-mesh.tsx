import React, { useLayoutEffect, useRef, useMemo } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import { useMapStore } from '../../model/map-store';
import { VOXEL_CONFIG } from '@pixelet/shared/entities/voxel/voxel-config';
import type { MapPoi } from '@pixelet/shared/entities/osm/model/osm-feature';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';

// 🎯 [Instance-Config]
const TEMP_OBJ = new THREE.Object3D();
const COLOR_NIGHT = new THREE.Color('#FFF59D'); // 연한 노란색
const COLOR_DAY = new THREE.Color('#555555');

const { TILE_SIZE, CELL_SIZE, Z_SCALE_RATIO, TILE_RESOLUTION } = OSM_CONSTANTS;

interface PoiInstancedMeshProps {
  pois: MapPoi[]; // 10,000개 데이터
}

/**
 * [Helper] 지형 높이 조회 (O(1) Access)
 * PoiItem에 있던 로직을 재사용하여 Mesh 높이를 맞춥니다.
 */
function getTerrainAltitude(absX: number, absY: number, tiles: any): number {
  const tx = Math.floor(absX / TILE_SIZE);
  const ty = Math.floor(absY / TILE_SIZE);
  const key = `${tx}:${ty}`;
  const tile = tiles[key];
  if (!tile) return 0;

  const localX = absX - tile.origin.x;
  const localY = absY - tile.origin.y;
  const gx = Math.floor(localX / CELL_SIZE);
  const gy = Math.floor((TILE_SIZE - localY) / CELL_SIZE);

  if (gx < 0 || gx >= TILE_RESOLUTION || gy < 0 || gy >= TILE_RESOLUTION)
    return 0;

  const index = gy * TILE_RESOLUTION + gx;
  return (tile.grids.terrain.height[index] || 0) * CELL_SIZE * Z_SCALE_RATIO;
}

export const PoiInstancedMesh = React.memo(
  ({ pois }: PoiInstancedMeshProps) => {
    const meshRef = useRef<THREE.InstancedMesh>(null);
    const groupRef = useRef<THREE.Group>(null);
    const tiles = useMapStore((state) => state.tiles); // 지형 데이터 접근

    // 🎯 [Optimization] 10,000개의 로컬 위치와 높이를 미리 계산 (Mount 시 1회)
    // 매 프레임 계산하면 CPU 터집니다.
    const instancesData = useMemo(() => {
      return pois.map((poi) => {
        // 1. 높이 계산
        const h = getTerrainAltitude(
          poi.center.x + VOXEL_CONFIG.HALF,
          poi.center.y + VOXEL_CONFIG.HALF,
          tiles,
        );

        return {
          id: poi.id,
          x: poi.center.x,
          y: poi.center.y,
          z: h + 1.5, // 지면보다 살짝 더 띄움 (시인성)
          category: poi.category,
        };
      });
    }, [pois, tiles]); // tiles가 로드되면 높이 재계산

    // 🎯 [Initial-Layout] 매트릭스 구성
    useLayoutEffect(() => {
      if (!meshRef.current) return;

      // 초기에는 모든 인스턴스를 (0,0,0)에 숨겨둠 -> useFrame에서 위치 잡음
      // 혹은 여기서 상대좌표를 미리 잡아도 되지만, Floating Origin 때문에
      // Group을 움직이는게 훨씬 쌉니다.
      instancesData.forEach((data, i) => {
        TEMP_OBJ.position.set(0, 0, 0); // 일단 0으로 초기화
        TEMP_OBJ.scale.setScalar(0); // 안보이게 시작
        TEMP_OBJ.updateMatrix();
        meshRef.current!.setMatrixAt(i, TEMP_OBJ.matrix);
      });
      meshRef.current.instanceMatrix.needsUpdate = true;
    }, [instancesData]);

    /**
     * [Frame-Loop]
     * InstancedMesh 전체를 Floating Origin에 맞춰 이동시킵니다.
     * 개별 점들을 움직이는게 아니라, '점들의 우주(Group)'를 통째로 옮깁니다.
     */
    useFrame((state) => {
      if (!groupRef.current || !meshRef.current) return;

      const currentOrigin = useMapStore.getState().origin;
      const cameraPos = state.camera.position;

      // 1. Group 이동 (Floating Origin 적용)
      // InstancedMesh 내부의 점들은 "절대 좌표 기준 상대값"으로 배치하고
      // Group을 -origin 만큼 이동시킵니다.
      // 하지만 InstancedMesh 특성상, 각 인스턴스의 position을 매번 바꾸는건 비쌈.
      // -> 💡 전략: Group을 (0,0,0)에 두고, 인스턴스를 (poi - origin)에 배치? (매 프레임 업데이트 필요 - 비쌈)
      // -> 💡 전략: ShaderMaterial에서 WorldPosition 보정? (복잡함)
      // -> 💡 최선: Group 포지션을 (-origin.x, 0, origin.y)로 이동?
      //    Three.js 좌표계 변환(Y <-> -Z) 때문에 헷갈리지만,
      //    가장 쉬운건 "매 프레임 Culling & Matrix Update"를 'Chunk' 단위로 하는 것입니다.

      // 여기서는 "간이 LOD"를 적용하여, 카메라 근처만 Scale 0 (HTML에 양보)으로 만들고
      // 나머지는 Scale 1로 만듭니다.

      let needsUpdate = false;

      instancesData.forEach((data, i) => {
        // 1. 로컬 좌표 계산 (이 연산은 10k번 돌아도 가벼움)
        const lx = data.x - currentOrigin.x + VOXEL_CONFIG.HALF;
        const lz = -(data.y - currentOrigin.y + VOXEL_CONFIG.HALF);

        // 2. 카메라 거리 계산
        const dx = lx - cameraPos.x;
        const dz = lz - cameraPos.z;
        const distSq = dx * dx + dz * dz;

        // 3. LOD 로직
        // 50m 이내(HTML 영역)거나 3.5km 밖이면 숨김
        const isNear = distSq < 50 * 50;
        const isFar = distSq > 3500 * 3500;

        // 가시성 결정
        const scale = isNear || isFar ? 0 : 1;

        // 행렬 업데이트
        TEMP_OBJ.position.set(lx, data.z, lz);
        // 빌보드 효과 (카메라 보기) - 옵션
        TEMP_OBJ.lookAt(cameraPos);
        TEMP_OBJ.scale.setScalar(scale * 2); // 점 크기 2m
        TEMP_OBJ.updateMatrix();

        meshRef.current!.setMatrixAt(i, TEMP_OBJ.matrix);
      });

      meshRef.current.instanceMatrix.needsUpdate = true;
    });

    return (
      <group ref={groupRef}>
        <instancedMesh
          ref={meshRef}
          args={[undefined, undefined, pois.length]}
          frustumCulled={false} // 우리가 직접 거리 계산하므로 끔
        >
          <circleGeometry args={[0.5, 8]} />
          <meshBasicMaterial
            color={COLOR_NIGHT}
            transparent
            opacity={0.8}
            depthWrite={false} // 투명도 문제 해결
            side={THREE.DoubleSide}
          />
        </instancedMesh>
      </group>
    );
  },
);

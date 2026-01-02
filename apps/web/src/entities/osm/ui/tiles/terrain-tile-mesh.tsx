import React, { useState, useLayoutEffect, useMemo, useRef } from 'react';
import * as THREE from 'three';
import { useThree, useFrame } from '@react-three/fiber';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import { useRelativePosition } from '../../lib/use-coordinate-converter';
import type {
  Tile,
  TileChannels,
} from '@pixelet/shared/entities/osm/model/tiles';
import { TILE_CHANNEL_KEYS } from '@pixelet/shared/entities/osm/model/tiles';
import { THREE_CONFIG } from '../../config/three-config';
import { toThreeCoordWithOffset } from '@/shared/lib/three/three-coords';
import { getTileBbox } from '@pixelet/shared/entities/osm/lib/utils/tile-utils';
import {
  createChannelTexture,
  createTerrainTexture,
} from '../../lib/render/texture-factory';
import { createTileMaterial } from '../../lib/render/tile-material';

const { TILE_SIZE, TILE_RES } = OSM_CONSTANTS;

// 🎯 [Helper] 점(Point)과 직사각형(Box) 사이의 최단 거리 계산 함수
// 점이 박스 안에 있으면 거리는 0입니다.
function getDistanceToTile(
  cameraPos: THREE.Vector3,
  tileCenter: { x: number; y: number },
  tileSize: number,
): number {
  const halfSize = tileSize / 2;

  // 타일의 Min/Max 경계
  const minX = tileCenter.x - halfSize;
  const maxX = tileCenter.x + halfSize;
  const minY = tileCenter.y - halfSize;
  const maxY = tileCenter.y + halfSize;

  // 카메라 위치를 타일 박스 영역으로 클램핑(Clamping)
  // dx, dy는 "박스 경계선에서 카메라까지의 거리"가 됨
  const dx = Math.max(minX - cameraPos.x, 0, cameraPos.x - maxX);
  const dy = Math.max(minY - cameraPos.y, 0, cameraPos.y - maxY);

  // 유클리드 거리 반환
  return Math.sqrt(dx * dx + dy * dy);
}

interface TerrainTileMeshProps {
  tile: Tile;
}

/**
 * [TerrainTileMesh]
 * 타일 기반 지형의 최소 렌더링 단위입니다.
 * InstancedMesh를 활용하여 지형 고도 및 채널 데이터를 가시화하며,
 * 거리 기반 Bounding Box LOD 시스템을 내장하고 있습니다.
 */
export const TerrainTileMesh = React.memo(({ tile }: TerrainTileMeshProps) => {
  const { coord, terrain } = tile;
  const { channels, elevation } = terrain;

  // 🎯 [Resolution Check] 4096m / 2048px = 2m Voxel
  const DATA_RES = TILE_RES;
  const voxelSize = TILE_SIZE / DATA_RES;

  const [currentLod, setCurrentLod] = useState(4);
  const { camera } = useThree();
  const frameCounter = useRef(Math.floor(Math.random() * 20));

  const absoluteCenter = useMemo(() => {
    const { minX, minY } = getTileBbox(coord);
    return { x: minX + TILE_SIZE / 2, y: minY + TILE_SIZE / 2, z: 0 };
  }, [coord]);

  const relativePosition = useRelativePosition(absoluteCenter);

  // 기본 지오메트리 (1x1x1)
  const sharedGeometry = useMemo(() => new THREE.BoxGeometry(1, 1, 1), []);

  useFrame(() => {
    frameCounter.current += 1;
    // 10프레임마다 LOD 체크
    if (frameCounter.current % 10 === 0) {
      // 🎯 [Logic Change] 중심점 거리 -> Bounding Box 최단 거리
      // 이제 타일 '안'에 있으면 distance는 0이 됩니다.
      const distance = getDistanceToTile(
        camera.position,
        { x: relativePosition.x, y: relativePosition.y },
        TILE_SIZE,
      );

      let nextLod = 1;

      // 🚀 [Threshold] 이제 거리가 0(내부)이거나 가까우면 확실하게 LOD 1 보장
      if (distance > 5000)
        nextLod = 8; // 5km 밖
      else if (distance > 2500)
        nextLod = 4; // 2.5km 밖
      else if (distance > 1200)
        nextLod = 2; // 1.2km 밖
      else nextLod = 1; // 1.2km 이내 (현재 타일 및 인접 타일)

      if (nextLod !== currentLod) {
        setCurrentLod(nextLod);
      }
    }
  });

  const lodRes = DATA_RES / currentLod;
  const instanceCount = lodRes * lodRes;

  const channelTextures = useMemo(() => {
    const textures = {} as Record<keyof TileChannels, THREE.DataTexture>;
    TILE_CHANNEL_KEYS.forEach((key) => {
      const tex = createChannelTexture(channels[key]);
      // ID맵은 Nearest 유지 (TextureFactory 설정과 이중 안전장치)
      tex.magFilter = THREE.NearestFilter;
      tex.minFilter = THREE.NearestFilter;
      textures[key] = tex;
    });
    return textures;
  }, [channels]);

  const terrainTexture = useMemo(() => {
    const tex = createTerrainTexture(elevation);
    tex.magFilter = THREE.NearestFilter;
    tex.minFilter = THREE.NearestFilter;
    return tex;
  }, [elevation]);

  const material = useMemo(() => {
    return createTileMaterial({
      channelTextures,
      terrainTexture,
      uResolution: new THREE.Vector2(DATA_RES, DATA_RES),
      uVoxelSize: voxelSize,
      uLodScale: currentLod,
      // 🚀 [Inject] 그리드 스타일 설정 주입
      gridConfig: {
        uBorderColor: new THREE.Color('#4fd1c5'), // Cyan (TRON 스타일)
        uBorderWidth: 0.05, // 0.0 ~ 0.5 (두께)
        uBorderOpacity: 0.5, // 투명도
      },
    });
  }, [channelTextures, terrainTexture, currentLod, voxelSize, DATA_RES]);

  React.useEffect(() => {
    return () => {
      Object.values(channelTextures).forEach((tex) => tex.dispose());
      terrainTexture.dispose();
      material.dispose();
    };
  }, [channelTextures, terrainTexture, material]);

  const meshRef = useRef<THREE.InstancedMesh>(null);

  // 🎯 [Restore useLayoutEffect]
  // 메쉬가 DOM에 반영된 직후(Layout) 실행되어 깜빡임 없이 Bounding Sphere를 주입합니다.
  // LOD가 바뀌어(key 변경) 메쉬가 재생성될 때마다 실행됩니다.
  useLayoutEffect(() => {
    if (meshRef.current) {
      // 타일 전체를 커버하는 거대한 구 생성 (반지름 약 4500m)
      const radius = Math.sqrt(2 * (TILE_SIZE / 2) ** 2) * 1.5;

      meshRef.current.geometry.boundingSphere = new THREE.Sphere(
        new THREE.Vector3(0, 0, 0),
        radius,
      );
    }
  }, [currentLod]); // LOD 변경 시 반드시 재실행

  return (
    <instancedMesh
      ref={meshRef}
      key={`tile-${currentLod}`}
      args={[sharedGeometry, undefined, instanceCount]}
      position={toThreeCoordWithOffset(relativePosition, -0.01)}
      rotation={THREE_CONFIG.LAYOUT.FLOOR_ROTATION as unknown as THREE.Euler}
      frustumCulled={true} // Bounding Sphere가 확실하므로 켜도 안전함
    >
      <primitive object={material} attach="material" />
    </instancedMesh>
  );
});

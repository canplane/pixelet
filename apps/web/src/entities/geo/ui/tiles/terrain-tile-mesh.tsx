import React, { useState, useLayoutEffect, useMemo, useRef } from 'react';
import * as THREE from 'three';
import { useThree, useFrame } from '@react-three/fiber';
import { GEO_CONSTANTS } from '@pixelet/core/entities/geo/model/geo-constants';
import { useRelativePosition } from '../../lib/use-coordinate-converter';
import type {
  Tile,
  TileChannels,
} from '@pixelet/core/entities/geo/model/tiles';
import { TILE_CHANNEL_KEYS } from '@pixelet/core/entities/geo/model/tiles';
import { THREE_CONFIG } from '../../config/three-config';
import { toThreeCoordWithOffset } from '@/shared/lib/three/three-coords';
import { getTileBbox } from '@pixelet/core/entities/geo/lib/utils/tile-utils';
import {
  createChannelTexture,
  createElevationTexture,
} from '../../lib/render/terrain/texture-factory';
import { createTileMaterial } from '../../lib/render/terrain/tile-material';

const { TILE_SIZE, TILE_RES } = GEO_CONSTANTS;

// ⚡ [Optimization] 전역 공유 자원 (인스턴스마다 생성 방지)
const SHARED_GEOMETRY = new THREE.BoxGeometry(1, 1, 1);
const TILE_RES_VEC2 = new THREE.Vector2(TILE_RES, TILE_RES);

// ⚡ [Optimization] LOD 임계값 제곱 미리 계산 (sqrt 제거용)
const LOD_THRESHOLDS_SQ = {
  L8: 5000 ** 2,
  L4: 2500 ** 2,
  L2: 1200 ** 2,
};

// 🎨 [Config] 사용자가 찾은 최적값 (0.1)
const GRID_CONFIG = {
  uBorderColor: new THREE.Color('#000000'),
  uBorderOpacity: 0.1,
};

// 🎯 [Helper] Box와의 최단 거리 '제곱' 계산 (sqrt 제거)
function getDistanceSqToTile(
  cameraPos: THREE.Vector3,
  tileCenter: { x: number; y: number },
  tileSize: number,
): number {
  const halfSize = tileSize / 2;
  const minX = tileCenter.x - halfSize;
  const maxX = tileCenter.x + halfSize;
  const minY = tileCenter.y - halfSize;
  const maxY = tileCenter.y + halfSize;

  const dx = Math.max(minX - cameraPos.x, 0, cameraPos.x - maxX);
  const dy = Math.max(minY - cameraPos.y, 0, cameraPos.y - maxY);

  return dx * dx + dy * dy;
}

interface TerrainTileMeshProps {
  tile: Tile;
}

export const TerrainTileMesh = React.memo(({ tile }: TerrainTileMeshProps) => {
  const { coord, terrain } = tile;
  const { channels, elevation } = terrain;

  const voxelSize = TILE_SIZE / TILE_RES;
  const [currentLod, setCurrentLod] = useState(4);
  const { camera } = useThree();
  const frameCounter = useRef(Math.floor(Math.random() * 20));

  const absoluteCenter = useMemo(() => {
    const { minX, minY } = getTileBbox(coord);
    return { x: minX + TILE_SIZE / 2, y: minY + TILE_SIZE / 2, z: 0 };
  }, [coord]);

  const relativePosition = useRelativePosition(absoluteCenter);

  useFrame(() => {
    frameCounter.current += 1;
    if (frameCounter.current % 10 === 0) {
      // ⚡ [Optimization] 거리 제곱 비교
      const distSq = getDistanceSqToTile(
        camera.position,
        { x: relativePosition.x, y: relativePosition.y },
        TILE_SIZE,
      );

      let nextLod = 1;
      if (distSq > LOD_THRESHOLDS_SQ.L8) nextLod = 8;
      else if (distSq > LOD_THRESHOLDS_SQ.L4) nextLod = 4;
      else if (distSq > LOD_THRESHOLDS_SQ.L2) nextLod = 2;
      else nextLod = 1;

      if (nextLod !== currentLod) {
        setCurrentLod(nextLod);
      }
    }
  });

  const instanceCount = (TILE_RES / currentLod) ** 2;

  const channelTextures = useMemo(() => {
    const textures = {} as Record<keyof TileChannels, THREE.DataTexture>;
    TILE_CHANNEL_KEYS.forEach((key) => {
      const tex = createChannelTexture(channels[key]);

      // ❌ [DELETE] 이 코드가 밉맵을 꺼버려서 자글거렸던 것임!
      // tex.magFilter = THREE.NearestFilter;
      // tex.minFilter = THREE.NearestFilter;

      textures[key] = tex;
    });
    return textures;
  }, [channels]);

  const elevationTexture = useMemo(() => {
    const tex = createElevationTexture(elevation);
    tex.magFilter = THREE.NearestFilter;
    tex.minFilter = THREE.NearestFilter;
    return tex;
  }, [elevation]);

  const material = useMemo(() => {
    return createTileMaterial({
      channelTextures,
      elevationTexture,
      uResolution: TILE_RES_VEC2,
      uVoxelSize: voxelSize,
      uLodScale: currentLod,
      gridConfig: GRID_CONFIG, // ⚡ 상수 재사용
    });
  }, [channelTextures, elevationTexture, currentLod, voxelSize]);

  React.useEffect(() => {
    return () => {
      Object.values(channelTextures).forEach((tex) => tex.dispose());
      elevationTexture.dispose();
      material.dispose();
    };
  }, [channelTextures, elevationTexture, material]);

  const meshRef = useRef<THREE.InstancedMesh>(null);

  useLayoutEffect(() => {
    if (meshRef.current) {
      const radius = Math.sqrt(2 * (TILE_SIZE / 2) ** 2) * 1.5;
      meshRef.current.geometry.boundingSphere = new THREE.Sphere(
        new THREE.Vector3(0, 0, 0),
        radius,
      );
    }
  }, [currentLod]);

  return (
    <instancedMesh
      ref={meshRef}
      key={`tile-${currentLod}`}
      args={[SHARED_GEOMETRY, undefined, instanceCount]}
      position={toThreeCoordWithOffset(relativePosition, -0.01)}
      rotation={THREE_CONFIG.LAYOUT.FLOOR_ROTATION as unknown as THREE.Euler}
      frustumCulled={true}
    >
      <primitive object={material} attach="material" />
    </instancedMesh>
  );
});

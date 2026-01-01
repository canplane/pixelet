import React, { useMemo, useEffect } from 'react';
import * as THREE from 'three';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import { useRelativePosition } from '../../lib/use-coordinate-converter';
import type {
  Tile,
  TileChannels,
} from '@pixelet/shared/entities/osm/model/tiles';
import { TILE_CHANNEL_KEYS } from '@pixelet/shared/entities/osm/model/tiles'; // 🎯 SSOT
import { THREE_CONFIG } from '../../config/three-config';
import { toThreeCoordWithOffset } from '@/shared/lib/three/three-coords';
import {
  getTileBbox,
  getTileKey,
} from '@pixelet/shared/entities/osm/lib/utils/tile-utils';
import {
  createChannelTexture,
  createTerrainTexture,
} from '../../lib/render/texture-factory';
import { createTileMaterial } from '../../lib/render/tile-material';

const { TILE_SIZE } = OSM_CONSTANTS;
interface TileViewProps {
  tile: Tile;
}

export const GeometryTileView = React.memo(({ tile }: TileViewProps) => {
  const { coord, grids } = tile;
  const { channels, terrain } = grids;

  // 1. 타일 중심점 및 상대 좌표 계산
  const absoluteCenter = useMemo(() => {
    const { minX, minY } = getTileBbox(coord);
    return { x: minX + TILE_SIZE / 2, y: minY + TILE_SIZE / 2, z: 0 };
  }, [coord]);

  // React Cycle에 묶여 있음 (origin 변경 시 재계산됨)
  const relativePosition = useRelativePosition(absoluteCenter);

  // 2. [Texture Factory] 모든 채널 텍스처 일괄 생성
  const channelTextures = useMemo(() => {
    const textures = {} as Record<keyof TileChannels, THREE.DataTexture>;

    // 🎯 SSOT(TILE_CHANNEL_KEYS) 기반 루프: sea, base, water... 자동 처리
    TILE_CHANNEL_KEYS.forEach((key) => {
      textures[key] = createChannelTexture(channels[key]);
    });

    return textures;
  }, [channels]); // channels 객체 자체가 변경될 때만 재생성

  // Terrain 텍스처 생성
  const terrainTexture = useMemo(
    () => createTerrainTexture(terrain.elevation),
    [terrain.elevation],
  );

  // 3. [Material Factory] 쉐이더 머티리얼 조립
  const material = useMemo(() => {
    return createTileMaterial({
      channelTextures,
      terrainTexture,
    });
  }, [channelTextures, terrainTexture]);

  // 4. [Lifecycle] 일괄 자원 해제 (Memory Leak 방지)
  useEffect(() => {
    return () => {
      // 🎯 루프를 돌며 모든 텍스처 해제
      Object.values(channelTextures).forEach((tex) => tex.dispose());
      terrainTexture.dispose();
      material.dispose();
    };
  }, [channelTextures, terrainTexture, material]);

  return (
    <mesh
      name={`tile-${getTileKey(coord)}`}
      position={toThreeCoordWithOffset(relativePosition, -0.01)}
      rotation={THREE_CONFIG.LAYOUT.FLOOR_ROTATION as unknown as THREE.Euler}
      receiveShadow
    >
      <planeGeometry args={[TILE_SIZE, TILE_SIZE, 32, 32]} />
      <primitive object={material} attach="material" />
    </mesh>
  );
});

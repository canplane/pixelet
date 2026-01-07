import React, { useMemo } from 'react';
import { useMapStore } from '@/entities/geo/model/map-store';
import { TerrainTileMesh } from '../tiles/terrain-tile-mesh';
import { useTileScheduler } from '@/entities/geo/lib/use-tile-scheduler';
import { getTileKey } from '@pixelet/core/entities/geo/lib/utils/tile-utils';
import { useTileStore } from '@/entities/geo/model/tile-store';

export const BaseLayer = React.memo(() => {
  const tileMap = useTileStore((state) => state.tileMap);
  const status = useMapStore((state) => state.status);

  // 타일 스케줄러 (데이터 페칭 트리거)
  useTileScheduler();

  const tiles = useMemo(() => Array.from(tileMap.values()), [tileMap]);

  // 로딩 중이고 데이터가 없으면 렌더링 스킵
  if (tiles.length === 0 && status === 'loading') return null;

  return (
    <group name="terrain-layer">
      {/* 🏔️ 지형 타일 리스트: 각 타일은 내부적으로 LOD/Geometry를 제어함 */}
      {tiles.map((tile) => (
        <TerrainTileMesh key={getTileKey(tile.coord)} tile={tile} />
      ))}
    </group>
  );
});

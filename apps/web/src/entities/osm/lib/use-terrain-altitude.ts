// src/entities/osm/lib/use-height-lookup.ts

import { useCallback } from 'react';
import { OSM_CONSTANTS } from '@pixelet/core/entities/osm/model/osm-constants';
import { useTileStore } from '@/entities/osm/model/tile-store';
import {
  getTerrainAltitudeCoord as _getTerrainAltitudeCoord,
  getTileKey,
  worldToTile,
} from '@pixelet/core/entities/osm/lib/utils/tile-utils';
import type { Coord2D } from '@pixelet/core/shared/model/coords';

const { CELL_SIZE } = OSM_CONSTANTS;

export function useTerrainAltitude() {
  const tilesMap = useTileStore((state) => state.tilesMap);

  // 단위: 셀 유닛
  const getTerrainAltitudeCoord = useCallback(
    (globalPos: Coord2D): number => {
      const { tileCoord, cellCoord } = worldToTile(globalPos);

      const tileKey = getTileKey(tileCoord); // z는 타일 키 생성 시 보통 무시되거나 0
      const tile = tilesMap.get(tileKey);
      if (!tile) return 0; // 타일이 아직 로드 안 됐으면 바닥(0) 반환

      return _getTerrainAltitudeCoord(tile, cellCoord);
    },
    [tilesMap],
  );

  // 단위: 미터
  const getTerrainAltitude = useCallback(
    (globalPos: Coord2D): number => {
      const elevationCoord = getTerrainAltitudeCoord(globalPos);
      return elevationCoord * CELL_SIZE;
    },
    [tilesMap],
  );

  return { getTerrainAltitudeCoord, getTerrainAltitude };
}

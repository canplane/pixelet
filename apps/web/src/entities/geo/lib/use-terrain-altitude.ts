// src/entities/geo/lib/use-terrain-altitude.ts

import { useCallback } from 'react';
import { GEO_CONSTANTS } from '@pixelet/core/entities/geo/model/geo-constants';
import { useTileStore } from '@/entities/geo/model/tile-store';
import { getTileKey, worldToTile } from '@pixelet/core/entities/geo/lib/utils/tile-utils';
import type { Coord2D } from '@pixelet/core/shared/model/coords';

const { CELL_SIZE } = GEO_CONSTANTS;

export function useTerrainAltitude() {
  const getTile = useTileStore((state) => state.getTile);

  /**
   * [getTerrainAltitudeCoord]
   * 단위: 셀 유닛 (Cell Units)
   */
  const getTerrainAltitudeCoord = useCallback(
    (wPos: Coord2D): number => {
      const { tileCoord, cellCoord } = worldToTile(wPos);
      const tileKey = getTileKey(tileCoord); // z는 타일 키 생성 시 보통 무시되거나 0

      // 1. [Data Acquisition]
      // 🚀 스토어에서 타일 데이터를 가져옵니다.
      const tile = getTile(tileKey);

      // 2. [Direct Sparse Access]
      // 🛡️ 타일이 로드되지 않았거나 고도 데이터(ele)가 없으면 0 반환
      if (!tile || !tile.base.ele) return 0;

      /**
       * [Architecture Point]
       * tile.base.ele는 SparseQuadtree 인스턴스입니다.
       * 팩토리에서 이미 Skeleton이 잡혀있으므로, 별도의 유효성 검사 없이
       * 쿼드트리의 위계 탐색(get)을 통해 Uniform/Detail 값을 즉시 획득합니다.
       */
      return tile.base.ele.get(cellCoord.cx, cellCoord.cy);
    },
    [getTile],
  );

  /**
   * [getTerrainAltitude]
   * 단위: 미터 (Meters)
   */
  const getTerrainAltitude = useCallback(
    (globalPos: Coord2D): number => {
      const elevationCoord = getTerrainAltitudeCoord(globalPos);
      // 고도 값에 셀 사이즈를 곱해 실제 미터 단위 높이로 변환
      return elevationCoord * CELL_SIZE;
    },
    [getTerrainAltitudeCoord],
  );

  return { getTerrainAltitudeCoord, getTerrainAltitude };
}

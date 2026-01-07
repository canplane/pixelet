import React, { useMemo } from 'react';
import { useTileStore } from '@/entities/geo/model/tile-store';
import { PoiManager } from '../pois/poi-manager';

export const PoiLayer = React.memo(() => {
  const tileMap = useTileStore((state) => state.tileMap);

  // 1. 타일 스토어에서 POI 데이터 수집
  // (useTileScheduler는 WorldLayer나 상위에서 한 번만 호출되면 됩니다)
  const tiles = useMemo(() => Array.from(tileMap.values()), [tileMap]);

  // 2. [Aggregate-POIs] 통합 로직 이관
  const allActivePois = useMemo(() => {
    const poiMap = new Map<string, any>();
    tiles.forEach((tile) => {
      tile.pois?.forEach((poi) => {
        if (!poiMap.has(poi.id)) poiMap.set(poi.id, poi);
      });
    });
    return Array.from(poiMap.values());
  }, [tiles]);

  if (allActivePois.length === 0) return null;

  return (
    <group name="poi-layer">
      {/* 🛰️ RTE Shader를 쓰는 똑똑한 녀석들은 여기 모입니다 */}
      {/* POI 매니저 (POI 렌더링 및 클러스터링/LOD 담당) */}
      {/* <PoiManager pois={allActivePois} /> */}
    </group>
  );
});

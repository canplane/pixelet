import React, { useMemo } from 'react';
import { useMapStore } from '@/entities/osm/model/map-store';
import { GeometryTileView } from '../tiles/geometry-tile-view';
import { PoiManager } from '../pois/poi-manager'; // 🎯 매니저 도입
import { useTileScheduler } from '@/entities/osm/lib/use-tile-scheduler';
import { getTileKey } from '@pixelet/shared/entities/osm/lib/utils/tile-utils';

/**
 * [WorldLayer]
 * 활성화된 타일들의 지형과 POI 데이터를 집계하여 하위 렌더러에 전달합니다.
 * 더 이상 여기서 거리 계산이나 필터링을 수행하지 않습니다. (LOD는 PoiManager의 역할)
 */
export const WorldLayer = React.memo(() => {
  const tiles = useMapStore((state) => state.tiles);
  const status = useMapStore((state) => state.status);

  // 타일 스케줄러 (데이터 페칭)
  useTileScheduler();

  // 1. 타일 리스트 변환
  const tileList = useMemo(() => Array.from(tiles.values()), [tiles]);

  /**
   * 2. [Aggregate-POIs]
   * 현재 로드된 모든 타일의 POI를 하나의 배열로 합칩니다.
   * 필터링(VISIBLE_RANGE)을 제거하여 원거리 POI도 Mesh로 표현될 수 있게 합니다.
   */
  const allActivePois = useMemo(() => {
    const poiMap = new Map<string, any>();

    tileList.forEach((tile) => {
      tile.pois?.forEach((poi) => {
        // 중복된 ID가 들어와도 마지막에 등록된 것으로 덮어씌워짐 (중복 제거)
        if (!poiMap.has(poi.id)) {
          poiMap.set(poi.id, poi);
        }
      });
    });

    return Array.from(poiMap.values());
  }, [tileList]);

  // 로딩 중이고 데이터가 없으면 렌더링 스킵
  if (tileList.length === 0 && status === 'loading') return null;

  return (
    <group name="osm-features-layer">
      {/* 1. 지형 지면 (타일) */}
      <group name="tiles-group">
        {tileList.map((tile) => (
          <GeometryTileView key={getTileKey(tile.coord)} tile={tile} />
        ))}
      </group>

      {/* 2. POI 매니저 (Hybrid LOD 시스템)
          🎯 10,000개 이상의 POI를 통째로 넘깁니다. */}
      {/* <PoiManager pois={allActivePois} /> */}
    </group>
  );
});

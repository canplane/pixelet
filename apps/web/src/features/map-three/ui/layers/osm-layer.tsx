import React, { useMemo } from 'react';
import { useMapStore } from '../../model/map-store';
import { OsmTileLayer } from './osm-tile-layer';
import { OsmPoiItem } from '../items/osm-poi-item';
import { useTileLifecycleManager } from '@/entities/osm/model/use-tile-lifecycle-manager';

/**
 * [osm-layer]
 * 활성화된 모든 타일의 지형 데이터와 POI를 통합 관리하는 컨테이너 레이어입니다.
 * 하위 컴포넌트들은 use-relative-position 훅을 통해 Floating Origin 보정을 수행합니다.
 */
export const OsmLayer = React.memo(() => {
  const center = useMapStore((state) => state.center);
  const worldTiles = useMapStore((state) => state.worldTiles);
  const status = useMapStore((state) => state.status);

  // 타일 라이프사이클 엔진 실행 (데이터 로드/해제 관리)
  useTileLifecycleManager(center);

  const tileList = useMemo(() => Array.from(worldTiles.values()), [worldTiles]);

  /**
   * 카메라 중심점(center) 거리 기반 가시권 POI 필터링
   * 절대 좌표 기준 거리를 계산하여 렌더링 부하를 줄입니다.
   */
  const visiblePois = useMemo(() => {
    if (!center || tileList.length === 0) return [];

    const VISIBLE_RANGE: Record<string, number> = {
      station: 2500,
      tourism: 2000,
      restaurant: 1200,
      cafe: 800,
      shop: 800,
      default: 1000,
    };

    const result: { key: string; poi: any }[] = [];

    tileList.forEach((tile) => {
      if (!tile.pois) return;

      tile.pois.forEach((poi) => {
        // 🎯 poi.center(절대 좌표)와 카메라 중심(절대 좌표) 간의 거리 계산
        const dx = poi.center.x - center.x;
        const dz = poi.center.z - center.z;
        const distSq = dx * dx + dz * dz;

        const maxDist = VISIBLE_RANGE[poi.category] || VISIBLE_RANGE.default;

        if (distSq < maxDist * maxDist) {
          result.push({
            key: `${tile.key}-${poi.id}`,
            poi: { ...poi, distSq },
          });
        }
      });
    });

    // 거리순 정렬 후 상위 40개만 렌더링 (성능 최적화)
    return result.sort((a, b) => a.poi.distSq - b.poi.distSq).slice(0, 40);
  }, [tileList, center]);

  if (tileList.length === 0 && status === 'loading') return null;

  return (
    <group name="osm-features-layer">
      {/* 1. 지형 지면 레이어 (Grid Shader) */}
      <group name="tiles-group">
        {tileList.map((tile) => (
          <OsmTileLayer key={tile.key} tile={tile} />
        ))}
      </group>

      {/* 2. POI 라벨 레이어 (Html Badge) */}
      <group name="poi-group">
        {visiblePois.map(({ key, poi }) => (
          <OsmPoiItem key={key} poi={poi} />
        ))}
      </group>
    </group>
  );
});

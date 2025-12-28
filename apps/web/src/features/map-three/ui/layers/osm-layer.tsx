import React, { useMemo } from 'react';
import { useMapStore } from '../../model/map-store';
import { OsmTileLayer } from './osm-tile-layer';
import { OsmPoiItem } from '../items/osm-poi-item';
import { useTileLifecycleManager } from '../../lib/use-tile-lifecycle-manager';

/**
 * [osm-layer]
 * 활성화된 모든 타일의 지형 데이터와 POI를 통합 관리하는 최상위 컨테이너입니다.
 * 하위 컴포넌트(Tile, POI)들은 use-relative-position 훅을 통해
 * Floating Origin(64비트 정밀도 문제 해결) 보정을 수행합니다.
 */
export const OsmLayer = React.memo(() => {
  const center = useMapStore((state) => state.center);
  const worldTiles = useMapStore((state) => state.worldTiles);
  const status = useMapStore((state) => state.status);

  /**
   * [Lifecycle] 타일 엔진 실행
   * 카메라 중심점(center) 변화를 감지하여 타일을 동적으로 로드/해제합니다.
   */
  useTileLifecycleManager();

  // Map 객체를 배열로 변환하여 렌더링 준비
  const tileList = useMemo(() => Array.from(worldTiles.values()), [worldTiles]);

  /**
   * [Optimization] 카메라 중심점 거리 기반 POI 필터링
   * 모든 POI를 렌더링하지 않고, 중요도(Category)에 따른 가시권 내 데이터만 선별합니다.
   * 지침: POI 아이템은 Floating Origin 대응을 위해 내부적으로 상대 좌표를 계산해야 합니다.
   */
  const visiblePois = useMemo(() => {
    if (!center || tileList.length === 0) return [];

    // 카테고리별 가시 거리 설정 (단위: meter)
    const VISIBLE_RANGE: Record<string, number> = {
      station: 2500, // 주요 거점은 멀리서도 보임
      tourism: 2000,
      university: 1800, // [추가] 시설 단지 거점
      hospital: 1800,
      restaurant: 1200,
      cafe: 800,
      shop: 800,
      default: 1000,
    };

    const result: { key: string; poi: any }[] = [];

    tileList.forEach((tile) => {
      if (!tile.pois) return;

      tile.pois.forEach((poi) => {
        // 🎯 절대 좌표(Absolute) 기준 거리 계산
        const dx = poi.center.x - center.x;
        const dy = poi.center.y - center.y;
        const distSq = dx * dx + dy * dy;

        const maxDist = VISIBLE_RANGE[poi.category] || VISIBLE_RANGE.default;

        // 가시 거리 이내에 있는 POI만 선별
        if (distSq < maxDist * maxDist) {
          result.push({
            key: `${tile.key}-${poi.id}`,
            poi: { ...poi, distSq },
          });
        }
      });
    });

    /**
     * [성능 제어]
     * 1. 가까운 순서로 정렬
     * 2. 최대 렌더링 개수를 제한(예: 40개)하여 HTML Overlay 부하 방지
     */
    return result.sort((a, b) => a.poi.distSq - b.poi.distSq).slice(0, 40);
  }, [tileList, center]);

  // 최초 로딩 시 데이터가 없으면 렌더링 스킵
  if (tileList.length === 0 && status === 'loading') return null;

  return (
    <group name="osm-features-layer">
      {/* 1. 지형 지면 레이어 (Grid Shader) 
          각 타일 내부에서 상대 좌표 변환이 수행됩니다. */}
      <group name="tiles-group">
        {tileList.map((tile) => (
          <OsmTileLayer key={tile.key} tile={tile} />
        ))}
      </group>

      {/* 2. POI 아이템 레이어 (Badge/Label) 
          지침에 따라 OsmPoiItem 내부에서 Floating Origin 대응 로직이 동작합니다. */}
      <group name="poi-group">
        {visiblePois.map(({ key, poi }) => (
          <OsmPoiItem key={key} poi={poi} />
        ))}
      </group>
    </group>
  );
});

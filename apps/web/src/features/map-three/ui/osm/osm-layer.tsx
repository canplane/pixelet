import React, { useMemo } from 'react';
import * as THREE from 'three';

import { useMapStore } from '../../model/map-store';
import { parseOsmData } from '../../lib/osm-parser';
import { THREE_CONFIG } from '../../model/three-config';

// 하위 컴포넌트들
import { OsmTileLayer } from './osm-tile-layer';
import { OsmPoiItem } from './osm-poi-item'; // POI 라벨용
import { useOsmDataFetcher } from '../../lib/use-osm-data-fetcher';

interface OsmFeaturesProps {
  options?: {
    showBuildings?: boolean;
    showRoads?: boolean;
    showPois?: boolean; // POI 표시 여부
  };
}

/**
 * OSM 데이터를 3D 피처(건물, 도로 등)로 변환하여 렌더링하는 레이어
 * 대량의 데이터를 효율적으로 관리하기 위해 필터링 및 거리 기반 컬링을 수행합니다.
 */
export const OsmLayer = React.memo(({ options }: OsmFeaturesProps) => {
  const {
    showBuildings = false,
    showRoads = true,
    showPois = true,
  } = options || {};

  // 지형/OSM 데이터를 가져오는 로직을 훅으로 캡슐화
  const { data } = useOsmDataFetcher();

  const center = useMapStore((state) => state.center);
  const project = useMapStore((state) => state.project);

  // 1. 데이터 파싱 (기존 로직 유지)
  const { infra, pois } = useMemo(() => {
    if (!data?.elements?.length) return { infra: [], pois: [] };
    try {
      const result = parseOsmData(data, project);
      const uniqueInfra = Array.from(
        new Map(result.infra.map((f) => [f.id, f])).values(),
      );
      const uniquePois = Array.from(
        new Map(result.pois.map((p) => [p.id, p])).values(),
      );
      return { infra: uniqueInfra, pois: uniquePois };
    } catch (error) {
      console.error('[OsmFeatures] Parsing failed:', error);
      return { infra: [], pois: [] };
    }
  }, [data, project]);

  // 2. 거리 기반 컬링 (가상 DOM 최적화 - 기준은 유지하되 묶음으로 보냅니다)
  const visibleData = useMemo(() => {
    if (!center) return { infra: [], pois: [] };
    const centerVec = new THREE.Vector3(center.x, 0, center.z);
    const renderDistance = (THREE_CONFIG.RENDERING.FADE_END || 3000) * 1.2;

    return {
      infra: infra.filter((f) => {
        if (f.type === 'building' && !showBuildings) return false;
        if (f.type === 'road' && !showRoads) return false;
        return centerVec.distanceTo(f.center) < renderDistance;
      }),
      pois: pois.filter((p) => {
        if (!showPois) return false;
        return centerVec.distanceTo(p.center) < renderDistance;
      }),
    };
  }, [infra, pois, center, showBuildings, showRoads, showPois]);

  if (!data) return null;

  return (
    <group name="osm-features-layer">
      {/* 🚀 1. 인프라 레이어 (건물 등) - 개별 루프 대신 통합 레이어 사용 */}
      {/* 이제 수천 개의 컴포넌트 대신 단 하나의 OsmTileLayer가 그려집니다. */}
      <OsmTileLayer infras={visibleData.infra} />

      {/* 2. POI 레이어 - POI는 개수가 상대적으로 적고 텍스트 처리가 필요하므로 일단 유지 */}
      <group name="poi-group">
        {visibleData.pois.map((p) => (
          <OsmPoiItem key={p.id} poi={p} />
        ))}
      </group>
    </group>
  );
});

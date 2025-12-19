// 1. 외부 라이브러리 (External Libraries)
import React, { useMemo, useEffect } from 'react';
import * as THREE from 'three';

// 2. 기능 전용 상태 관리 및 로직 (Feature Store & Lib)
import { useMapStore } from '../../model/map-store';
import { parseOsmData } from '../../lib/osm-parser';
import { THREE_CONFIG } from '../../model/three-config';

// 3. 하위 UI 컴포넌트 (Internal UI Components)
import { OsmFeatureItem } from './osm-feature-item';

interface OsmFeaturesProps {
  data: any;
  options?: {
    showBuildings?: boolean;
    showRoads?: boolean;
  };
}

/**
 * OSM 데이터를 3D 피처(건물, 도로 등)로 변환하여 렌더링하는 레이어
 * 대량의 데이터를 효율적으로 관리하기 위해 필터링 및 거리 기반 컬링을 수행합니다.
 */
export const OsmFeatures = React.memo(({ data, options }: OsmFeaturesProps) => {
  const { project, center } = useMapStore();

  const { showBuildings = false, showRoads = true } = options || {};

  /**
   * 1. OSM 데이터 파싱 및 절대 좌표 투영
   * [성능] 데이터 원본이 바뀔 때만 무거운 파싱 작업을 수행합니다.
   */
  const features = useMemo(() => {
    if (!data || !data.elements || data.elements.length === 0) return [];

    try {
      // 파싱 시 중복 ID 제거 (Overpass API의 중복 응답 방지)
      const parsed = parseOsmData(data, project);
      const uniqueFeatures = Array.from(
        new Map(parsed.map((f) => [f.id, f])).values(),
      );

      console.log(
        `[OsmFeatures] Parsed ${uniqueFeatures.length} unique features`,
      );
      return uniqueFeatures;
    } catch (error) {
      console.error('[OsmFeatures] Parsing failed:', error);
      return [];
    }
  }, [data, project]);

  /**
   * 2. 사용자 옵션 및 거리 기반 가시성 필터링 (Frustum/Distance Culling)
   * [최적화] 너무 멀리 있는 데이터는 아예 Virtual DOM에 생성하지 않아 메모리를 아낍니다.
   */
  const visibleFeatures = useMemo(() => {
    if (!center) return [];

    const centerVec = new THREE.Vector3(center.x, 0, center.z);
    // THREE_CONFIG에 설정된 FADE_END 거리보다 조금 더 넉넉하게(예: 1.2배) 필터링 범위를 잡습니다.
    const renderDistance = (THREE_CONFIG.RENDERING.FADE_END || 3000) * 1.2;

    return features.filter((f) => {
      // 1단계: 타입 필터링
      if (f.type === 'building' && !showBuildings) return false;
      if (f.type === 'road' && !showRoads) return false;

      // 2단계: 거리 필터링 (너무 먼 데이터는 렌더링 제외)
      // feature.center는 절대 좌표이므로 현재 지도 중심(center)과의 거리를 계산합니다.
      const dist = centerVec.distanceTo(f.center);
      return dist < renderDistance;
    });
  }, [features, center, showBuildings, showRoads]);

  /**
   * 3. 데이터 로드 상태 모니터링
   */
  useEffect(() => {
    if (visibleFeatures.length > 500) {
      console.warn(
        `[OsmFeatures] High load: Rendering ${visibleFeatures.length} features.`,
      );
    }
  }, [visibleFeatures.length]);

  if (visibleFeatures.length === 0) return null;

  return (
    <group name="osm-features-layer">
      {visibleFeatures.map((f) => (
        // key는 고유 id를 사용하여 리렌더링 최적화
        <OsmFeatureItem key={f.id} feature={f} />
      ))}
    </group>
  );
});

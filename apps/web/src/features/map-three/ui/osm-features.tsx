import React, { useMemo } from 'react';
import { useMapStore } from '@/entities/map/model/map.store';
import { parseOsmData } from '../lib/osm-parser';
import { OsmFeatureItem } from '../ui/osm-feature-item';

interface OsmFeaturesProps {
  data: any;
  options?: {
    showBuildings?: boolean;
    showRoads?: boolean;
  };
}

export const OsmFeatures = React.memo(({ data, options }: OsmFeaturesProps) => {
  const { project, origin } = useMapStore();

  const { showBuildings = false, showRoads = true } = options || {};

  // 1. 순수 데이터 파싱
  const features = useMemo(() => {
    if (!origin || !data) return [];
    return parseOsmData(data, project);
  }, [data, project, origin]);

  // 플래그에 따라 필터링된 리스트 생성
  const visibleFeatures = useMemo(() => {
    return features.filter((f) => {
      if (f.type === 'building') return showBuildings;
      if (f.type === 'road') return showRoads;
      return true;
    });
  }, [features, showBuildings, showRoads]);

  if (visibleFeatures.length === 0) return null;

  return (
    <group name="osm-features-layer">
      {visibleFeatures.map((f) => (
        <OsmFeatureItem key={f.id} feature={f} />
      ))}
    </group>
  );
});

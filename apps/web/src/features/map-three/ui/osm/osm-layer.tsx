import { useMapDataFetcher } from '../../lib/use-map-data-fetcher';
import { OsmFeatures } from './osm-features';

interface OsmLayerProps {
  showBuildings: boolean;
}

export function OsmLayer({ showBuildings }: OsmLayerProps) {
  // 지형/OSM 데이터를 가져오는 로직을 훅으로 캡슐화
  const { osmData } = useMapDataFetcher();

  if (!osmData) return null;

  return (
    <OsmFeatures
      data={osmData}
      options={{
        showBuildings,
        showRoads: true,
      }}
    />
  );
}

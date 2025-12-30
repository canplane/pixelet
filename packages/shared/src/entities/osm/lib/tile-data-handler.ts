/**
 * @file tile-data-handler.ts
 * 정규화된 OSM 데이터를 인프라 객체와 그리드 데이터로 변환하는 공용 핸들러입니다.
 */

import { parseOsmData } from './osm-parser';
import { bakeGeometryGrid } from './geometry-baker';
import type { MeterPos } from '@pixelet/shared/shared/model/coords';
import type { OsmFeature } from '../model/osm-feature';
import type { MapInfra, MapPoi } from '../model/osms';
import type { VoxelLayers } from '../model/osm-tile';

/**
 * [process-osm-to-grids]
 * Raw OSM 데이터를 인프라 객체와 그리드 데이터로 변환합니다.
 * 전처리기(Normalizer)를 거쳐 정규화된 데이터를 각 엔진 컴포넌트에 배분합니다.
 */
export function processOsmToGrids(
  elements: OsmFeature[], // 🎯 오직 정규화된 데이터만 수용
  origin: MeterPos,
  options?: { tileSize: number; cellSize: number },
): {
  infras: MapInfra[];
  pois: MapPoi[];
  partialLayers: Partial<VoxelLayers>;
} {
  /**
   * 1. 파싱: 'osm-parser' 실행
   * 정규화된 데이터를 바탕으로 3D 엔진용 인프라(Way/Rel) 및 POI(Node) 객체를 생성합니다.
   * 이미 정규화가 끝났으므로 연산 오버헤드 없이 분류와 정렬만 수행합니다.
   */
  const { infras, pois } = parseOsmData(elements);

  /**
   * 2. 그리드 베이킹 (정규화된 데이터 직결)
   * 인프라 객체(infras) 대신, 더 원원 정보가 풍부한 normalizedElements를 베이커에 직접 전달하여
   * 래스터화(Rasterization) 정밀도를 높입니다.
   */
  let partialLayers: Partial<VoxelLayers> = {};
  if (elements.length > 0) {
    const result = bakeGeometryGrid(elements, origin, options);
    partialLayers = result.layers;
  }

  return { infras, pois, partialLayers };
}

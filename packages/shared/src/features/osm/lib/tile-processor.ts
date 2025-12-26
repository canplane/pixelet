/**
 * @file tile-processor.ts
 * 단일 타일의 생애주기(데이터 로드 -> 가공 -> 그리드 생성)를 책임지는 원자적 함수입니다.
 * 모든 결과 데이터는 서버 및 글로벌 기준과 동일한 '절대 미터 좌표계'를 유지합니다.
 */

import { requestOverpass } from '@pixelet/shared/entities/osm/api/overpass.api.client';
import { GEOMETRY_FILTERS } from '@pixelet/shared/entities/osm/api/query/osm-geometry-queries';
// ✨ [New Import] 행정 구역 및 단지 정보 쿼리 추가
import { BOUNDARY_FILTERS } from '@pixelet/shared/entities/osm/api/query/osm-boundary-queries';
import { POI_QUERIES } from '@pixelet/shared/entities/osm/api/query/osm-poi-queries';
import { create_query } from '@pixelet/shared/entities/osm/api/overpass-utils';
import { projection } from '@pixelet/shared/shared/lib/map/projection';
import type { MapInfra, MapPoi } from '@pixelet/shared/entities/osm/model/osms';
import { parseOsmData } from '@pixelet/shared/entities/osm/lib/osm-parser';
import { bakeTileGrid } from './tile-baker';
import {
  createEmptyGrids,
  type TileGrids,
  type WorldTile,
} from '@pixelet/shared/entities/osm/model/tiles';
import { OSM_CONFIG } from '@pixelet/shared/entities/osm/osm-config';
import { getTileBbox } from '@pixelet/shared/entities/osm/lib/tile-math';
import type { Location, MeterPos } from '@pixelet/shared/shared/model/coords';
import type { OverpassElement } from '@pixelet/shared/entities/osm/api/overpass-types';
import { randomDelay } from '@pixelet/shared/shared/lib/aync-utils';

// [중요] 좌표 변환 함수 import
// 주의: 이 함수들은 DOM(window, document)에 의존하지 않는 순수 수학 함수여야 합니다.

const { project } = projection;

// --- Types ---
type StepName = 'Infrastructure' | 'Structure' | 'POI';

interface ProcessingStep {
  name: StepName;
  query: string;
  critical: boolean;
  targetChannels: (keyof TileGrids)[];
}

// --- Sub-Functions (Logic Extraction) ---

/**
 * [Step 1] 실행 계획 수립
 * BBox를 기반으로 3단계(Pass) 쿼리 계획을 생성합니다.
 * 순서: 지형 -> 물 -> 교통 -> 구조물 -> POI
 * * [고도화] Infrastructure 단계에 행정 구역 및 시설 단지(Boundary) 쿼리를 통합하여
 * 학교, 아파트 단지, 공원 구역 정보를 한 번에 확보합니다.
 */
function createProcessingSteps(bbox: string): ProcessingStep[] {
  // ✨ [통합 쿼리] 기본 지형뿐만 아니라 단지, 구역 정보를 한 번에 요청하여 API 호출 횟수를 최적화합니다.
  // [보강] create_query 내부에서 (._; > ;) 전략을 사용하여 모든 노드 좌표를 강제 포함합니다.
  const queryInfra = create_query(`
    ${GEOMETRY_FILTERS.BASE(bbox)}
    ${BOUNDARY_FILTERS.ADMIN(bbox)}
    ${BOUNDARY_FILTERS.RESIDENTIAL(bbox)}
    ${BOUNDARY_FILTERS.LEISURE(bbox)}
    ${GEOMETRY_FILTERS.WATER(bbox)}
    ${GEOMETRY_FILTERS.TRANSPORT(bbox)}
  `);

  const queryStructure = create_query(GEOMETRY_FILTERS.STRUCTURE(bbox));

  const poiFilter = POI_QUERIES.GET_RAW_FILTER
    ? POI_QUERIES.GET_RAW_FILTER(bbox)
    : '';
  const queryPoi = create_query(poiFilter);

  return [
    {
      name: 'Infrastructure',
      query: queryInfra,
      critical: true, // 지형 실패 시 타일 생성 불가
      targetChannels: ['base', 'water', 'transport'],
    },
    {
      name: 'Structure',
      query: queryStructure,
      critical: false,
      targetChannels: ['structure'],
    },
    {
      name: 'POI',
      query: queryPoi,
      critical: false,
      targetChannels: [], // POI는 복셀 그리드에 영향 없음
    },
  ];
}

/**
 * [Step 2] 데이터 가공 (Worker)
 * 수신된 Raw 데이터를 파싱하고, 필요한 경우 부분 베이킹(Partial Baking)을 수행하여
 * 메인 그리드(targetGrids)에 병합합니다.
 */
function processLayerData(
  elements: OverpassElement[],
  step: ProcessingStep,
  context: {
    origin: MeterPos;
    tileSize: number;
    cellSize: number;
    project: (loc: Location) => MeterPos | null;
  },
  targetGrids: TileGrids, // Mutable Update
): { newInfras: MapInfra[]; newPois: MapPoi[] } {
  /**
   * (A) 파싱: 'osm-parser' 실행
   * [보강] 'elements'에는 (._; > ;) 쿼리 결과로 포함된 다량의 순수 Node 좌표 데이터가 섞여 있습니다.
   * parseOsmData 내부의 nodeCache가 이 elements를 전체 순회하여 Way의 좌표를 100% 복구합니다.
   */
  const { infras, pois } = parseOsmData(elements, context.project);

  // (B) 부분 베이킹 & 채널 병합
  if (infras.length > 0 && step.targetChannels.length > 0) {
    const { grids: partialGrids } = bakeTileGrid(infras, context.origin, {
      tileSize: context.tileSize,
      cellSize: context.cellSize,
    });

    // 고속 메모리 복사 (Channel Merging)
    step.targetChannels.forEach((channel) => {
      // partialGrids가 생성되지 않은 채널이 있을 수 있으므로 방어 로직 추가
      if (partialGrids[channel]) {
        targetGrids[channel].set(partialGrids[channel]);
      }
    });
  }

  return { newInfras: infras, newPois: pois };
}

// --- Main Function ---

/**
 * [tile-processor]
 * 단일 타일의 생애주기(데이터 로드 -> 가공 -> 그리드 생성)를 책임지는 원자적 함수입니다.
 */
export async function fetchAndProcessTile(
  tx: number,
  tz: number,
  onProgress?: (partialTile: WorldTile) => void,
): Promise<WorldTile> {
  const { TILE_SIZE, CELL_SIZE, API_MODE } = OSM_CONFIG;
  const key = `${tx}:${tz}`;
  const resolution = Math.floor(TILE_SIZE / CELL_SIZE);

  // 1. Origin 설정 (절대 좌표계 기준점)
  const origin: MeterPos = {
    x: tx * TILE_SIZE,
    y: 0,
    z: tz * TILE_SIZE,
  };

  /**
   * 2. 타일 경계(BBox) 계산
   */
  const bbox = getTileBbox(tx, tz);

  // 3. 실행 계획 수립
  const steps = createProcessingSteps(bbox);

  // 4. 상태 저장소 초기화
  const currentGrids = createEmptyGrids(resolution);
  const accumulatedInfras: MapInfra[] = [];
  const accumulatedPois: MapPoi[] = [];

  // 5. 순차 실행 루프 (Sequential Execution)
  for (let i = 0; i < steps.length; i++) {
    const step = steps[i];
    if (!step.query || step.query.trim() === '') continue;

    try {
      const response = await requestOverpass(step.query);

      // [보강] response.elements에는 쿼리 필터 결과물(Way/Relation)과
      // 재귀적으로 추출된 좌표 데이터(Node)가 모두 포함되어 전달되어야 합니다.
      if (response?.elements && response.elements.length > 0) {
        const result = processLayerData(
          response.elements,
          step,
          { origin, tileSize: TILE_SIZE, cellSize: CELL_SIZE, project },
          currentGrids,
        );

        accumulatedInfras.push(...result.newInfras);
        accumulatedPois.push(...result.newPois);

        if (onProgress) {
          onProgress({
            key,
            origin,
            grids: currentGrids,
            infras: accumulatedInfras,
            pois: accumulatedPois,
            resolution,
          });
        }
      }
    } catch (error) {
      console.warn(
        `[TileProcessor] ${step.name} fetch failed in ${key}.`,
        error,
      );
      if (step.critical) throw error;
    }

    if (API_MODE === 'public' && i < steps.length - 1) {
      await randomDelay(500, 1500);
    }
  }

  return {
    key,
    origin,
    grids: currentGrids,
    infras: accumulatedInfras,
    pois: accumulatedPois,
    resolution,
  };
}

/**
 * @file tile-processor.ts
 * 단일 타일의 생애주기(데이터 로드 -> 가공 -> 그리드 생성)를 책임지는 원자적 함수입니다.
 * 모든 결과 데이터는 서버 및 글로벌 기준과 동일한 '절대 미터 좌표계'를 유지합니다.
 */

import { requestOverpass } from '../api/overpass.api';
import { GEOMETRY_FILTERS } from '../lib/query/osm-geometry-queries';
// ✨ [New Import] 행정 구역 및 단지 정보 쿼리 추가
import { BOUNDARY_FILTERS } from '../lib/query/osm-boundary-queries';
import { create_query } from '../lib/query/overpass-utils';
import { POI_QUERIES } from '../lib/query/osm-poi-queries';
import {
  parseOsmData,
  type MapInfra,
  type MapPoi,
} from '../lib/processing/osm-parser';
import {
  bakeTileGrid,
  createEmptyGrids,
  type TileGrids,
} from '../lib/baking/tile-baker';
import { OSM_CONFIG } from '../config/osm-config';
import { getTileBBox } from '../lib/utils/tile-math';
import type { Location, MeterPos } from '@/shared/types/map';
import type { WorldTile } from '@/features/map-three/model/map-store';
import type { OverpassElement } from './types';
import { randomDelay } from '@/shared/lib/aync-utils';

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
  // (A) 파싱: 'osm-parser'가 ID 중복 제거 및 객체 우선순위(Layer Stacking) 정리
  const { infras, pois } = parseOsmData(elements, context.project);

  // (B) 부분 베이킹 & 채널 병합
  // 데이터가 있고, 그리드에 영향을 주는 레이어인 경우에만 수행
  if (infras.length > 0 && step.targetChannels.length > 0) {
    const { grids: partialGrids } = bakeTileGrid(infras, context.origin, {
      tileSize: context.tileSize,
      cellSize: context.cellSize,
    });

    // 고속 메모리 복사 (Channel Merging)
    // 쉐이더의 레이어 합성 우선순위와 일치하도록 채널별 데이터를 병합합니다.
    step.targetChannels.forEach((channel) => {
      targetGrids[channel].set(partialGrids[channel]);
    });
  }

  return { newInfras: infras, newPois: pois };
}

// --- Main Function ---

/**
 * [tile-processor]
 * 단일 타일의 생애주기(데이터 로드 -> 가공 -> 그리드 생성)를 책임지는 원자적 함수입니다.
 * 모든 결과 데이터는 서버 및 글로벌 기준과 동일한 '절대 미터 좌표계'를 유지합니다.
 * * [주요 정책]
 * 1. 대형 쿼리 동시 실행 대신, 레이어별 순차 실행을 통해 API 안정성을 확보했습니다.
 * 2. 특정 레이어 로드 실패가 전체 타일 로딩 실패로 이어지지 않도록 방어 로직이 적용되었습니다.
 * 3. 점진적 업데이트(onProgress)를 통해 사용자가 로딩 과정을 실시간으로 확인할 수 있습니다.
 * * @param tx - 타일 격자 X 인덱스
 * @param tz - 타일 격자 Z 인덱스
 * @param project - 위경도를 미터로 변환하는 함수
 * @param unproject - 미터를 위경도로 변환하는 함수
 * @param onProgress - 단계별 렌더링을 위한 콜백 함수 (선택 사항)
 */
export async function fetchAndProcessTile(
  tx: number,
  tz: number,
  project: (loc: Location) => MeterPos | null,
  unproject: (pos: { x: number; y: number; z: number }) => Location,
  onProgress?: (partialTile: WorldTile) => void,
): Promise<WorldTile> {
  const { TILE_SIZE, CELL_SIZE, API_MODE } = OSM_CONFIG;
  const key = `${tx}:${tz}`;
  const resolution = Math.floor(TILE_SIZE / CELL_SIZE);

  // 1. Origin 설정
  const origin: MeterPos = {
    x: tx * TILE_SIZE,
    y: 0,
    z: tz * TILE_SIZE,
  };

  /**
   * 2. 타일 경계(BBox) 계산
   * getTileBBox 유틸리티를 사용하여 Grid Index 기반 위경도 범위를 도출합니다.
   */
  const bbox = getTileBBox(tx, tz, (pos) =>
    unproject({ x: pos.x, y: 0, z: pos.z }),
  );

  // 3. 실행 계획 수립 (3-Pass Strategy: Infra -> Structure -> POI)
  const steps = createProcessingSteps(bbox);

  // 4. 상태 저장소 초기화
  const currentGrids = createEmptyGrids(resolution);
  const accumulatedInfras: MapInfra[] = [];
  const accumulatedPois: MapPoi[] = [];

  // 5. 순차 실행 루프 (Sequential Execution)
  // Overpass API의 부하를 분산하고 타임아웃을 방지하기 위해 순차적으로 처리합니다.
  for (let i = 0; i < steps.length; i++) {
    const step = steps[i];
    if (!step.query || step.query.trim() === '') continue;

    try {
      const response = await requestOverpass(step.query);

      if (response?.elements && response.elements.length > 0) {
        // [위임] 복잡한 데이터 가공 및 병합 처리는 Worker 함수에게 위임
        const result = processLayerData(
          response.elements,
          step,
          { origin, tileSize: TILE_SIZE, cellSize: CELL_SIZE, project },
          currentGrids, // 참조 전달 (내부에서 업데이트됨)
        );

        // 결과 누적
        accumulatedInfras.push(...result.newInfras);
        accumulatedPois.push(...result.newPois);

        // [Progressive Rendering] 점진적 렌더링 알림
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

      // 인프라(지면/교통) 로딩 실패 시 타일 생성 불가 -> 에러 전파 (Critical)
      if (step.critical) throw error;
    }

    // [서버 매너] 공용 모드일 때 마지막 단계가 아니라면 랜덤 딜레이 적용
    if (API_MODE === 'public' && i < steps.length - 1) {
      await randomDelay(500, 1500);
    }
  }

  // 6. 가공 완료된 원자적 타일 데이터 반환 (WorldTile 인터페이스 준수)
  return {
    key,
    origin,
    grids: currentGrids,
    infras: accumulatedInfras,
    pois: accumulatedPois,
    resolution,
  };
}

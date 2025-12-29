/**
 * @file tile-public-processor.ts
 * Public Overpass API를 통해 OSM 데이터를 단계별로 Fetch하고,
 * 이를 복셀 그리드로 가공(Baking)하여 최종 타일 데이터를 생성합니다.
 */

import { requestOverpass } from '@pixelet/shared/entities/osm/api/overpass.api.client';
import { GEOMETRY_FILTERS } from '@pixelet/shared/entities/osm/api/query/osm-geometry-queries';
import { BOUNDARY_FILTERS } from '@pixelet/shared/entities/osm/api/query/osm-boundary-queries';
import { POI_QUERIES } from '@pixelet/shared/entities/osm/api/query/osm-poi-queries';
import { createQuery } from '@pixelet/shared/entities/osm/api/overpass-utils';
import type {
  GeometryGrids,
  VoxelLayers,
  GeometryTile,
} from '@pixelet/shared/entities/osm/model/tiles';
import { createEmptyGrids } from '@pixelet/shared/entities/osm/model/tiles';
import { OSM_CONFIG } from '@pixelet/shared/entities/osm/osm-config';
import {
  getTileBbox,
  unprojectBbox,
} from '@pixelet/shared/entities/osm/lib/tile-math';
import type { Bbox } from '@pixelet/shared/entities/osm/lib/tile-math';
import { normalizeOsmData } from '@pixelet/shared/entities/osm/lib/osm-normalizer'; // 🎯 추가: Raw -> Normalized 변환
import { processOsmToGrids } from '@pixelet/shared/entities/osm/lib/tile-data-handler';
import { randomDelay } from '@pixelet/shared/shared/lib/aync-utils';
import type { MeterPos } from '@pixelet/shared/shared/model/coords';
import type { MapInfra, MapPoi } from '@pixelet/shared/entities/osm/model/osms';
import type { TileWorkerInput } from '@pixelet/shared/entities/osm/model/worker-types';
import { requestElevation } from '@pixelet/shared/entities/elevation/api/elevation.api.client';
import { bakeElevationGrid } from '@pixelet/shared/entities/elevation/lib/elevation-baker';

const { TILE_SIZE, CELL_SIZE } = OSM_CONFIG;

type StepName = 'Infrastructure' | 'Structure' | 'POI';

interface ProcessingStep {
  name: StepName;
  query: string;
  critical: boolean; // 실패 시 전체 타일 생성을 중단할지 여부
  targetChannels: (keyof VoxelLayers)[];
}

/**
 * [Internal] 타일 BBox를 기반으로 단계별 Overpass QL 쿼리 계획을 수립합니다.
 * 순서: 지형 -> 물 -> 교통 -> 구조물 -> POI
 * * [고도화] Infrastructure 단계에 행정 구역 및 시설 단지(Boundary) 쿼리를 통합하여 학교, 아파트 단지, 공원 구역 정보를 한 번에 확보합니다.
 */
function createProcessingSteps(bbox: Bbox): ProcessingStep[] {
  const { minLat, minLng, maxLat, maxLng } = unprojectBbox(bbox);
  const bboxString = `${minLat},${minLng},${maxLat},${maxLng}`;

  // 1. 지형, 구역, 도로망 쿼리 (Infrastructure)
  // 기본 지형뿐만 아니라 단지, 구역 정보를 한 번에 요청하여 API 호출 횟수를 최적화합니다.
  // create_query 내부에서 (._; > ;) 전략을 사용하여 모든 노드 좌표를 강제 포함합니다.
  const queryInfra = createQuery(`
      ${GEOMETRY_FILTERS.BASE(bboxString)}
      ${BOUNDARY_FILTERS.ADMIN(bboxString)}
      ${BOUNDARY_FILTERS.RESIDENTIAL(bboxString)}
      ${BOUNDARY_FILTERS.LEISURE(bboxString)}
      ${GEOMETRY_FILTERS.WATER(bboxString)}
      ${GEOMETRY_FILTERS.TRANSPORT(bboxString)}
    `);

  // 2. 건물 구조체 쿼리 (Structure)
  const queryStructure = createQuery(GEOMETRY_FILTERS.STRUCTURE(bboxString));

  // 3. 관심 지점 쿼리 (POI)
  const poiFilter = POI_QUERIES.GET_RAW_FILTER
    ? POI_QUERIES.GET_RAW_FILTER(bboxString)
    : '';
  const queryPoi = createQuery(poiFilter);

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
 * [Helper 1] 개별 단계를 실행하여 가공된 데이터를 반환합니다.
 */
async function executeStep(step: ProcessingStep, origin: MeterPos) {
  const response = await requestOverpass(step.query);
  if (!response?.elements?.length) return null;

  /**
   * 정규화: 'osm-normalizer' 실행
   * [보강] Raw elements를 3857 Meter 좌표계로 변환하고,
   * isClosed 판정, Holes 조립, 물리 수치(Computed) 계산을 한 번에 끝냅니다.
   */
  const normalizedElements = normalizeOsmData(response.elements);

  // 정규화된 데이터를 핸들러에 전달 (서버의 GeoService 호출과 동일한 규격이 됨)
  return processOsmToGrids(normalizedElements, origin);
}

/**
 * [Helper 2] 가공된 데이터를 현재 타일 상태에 병합합니다.
 */
function mergeStepResult(
  target: { grids: GeometryGrids; infras: MapInfra[]; pois: MapPoi[] },
  source: {
    partialLayers: Partial<VoxelLayers>;
    infras: MapInfra[];
    pois: MapPoi[];
  },
  targetChannels: (keyof VoxelLayers)[],
) {
  target.infras.push(...source.infras);
  target.pois.push(...source.pois);

  targetChannels.forEach((channel) => {
    const channelData = source.partialLayers[channel];
    if (channelData) {
      target.grids.layers[channel].set(channelData);
    }
  });
}

/**
 * [Main Orchestrator] Public API 모드용 타일 프로세서
 * Overpass API 호출 -> 데이터 가공 -> 점진적 업데이트 -> 최종 타일 반환의 워크플로우를 관리합니다.
 */
export async function processPublicTile(
  coord: TileWorkerInput['coord'],
  origin: MeterPos,
  onProgress?: (partialTile: GeometryTile) => void,
): Promise<GeometryTile> {
  const { tx, ty } = coord;
  // 타일 당 그리드 해상도 (N x N)
  const resolution = Math.floor(TILE_SIZE / CELL_SIZE);

  const key = `${tx}:${ty}`;
  const bbox = getTileBbox(tx, ty);

  const steps = createProcessingSteps(bbox);
  const currentGrids = createEmptyGrids(resolution);

  // 🎯 [Elevation Integration]: 고도 데이터 수집 시작
  // OSM 쿼리들과 병렬로 실행하여 네트워크 대기 시간을 최소화합니다.
  const elevationTask = requestElevation(bbox);

  const accumulatedData = {
    grids: currentGrids,
    infras: [] as MapInfra[],
    pois: [] as MapPoi[],
  };

  // 1. 고도 데이터 먼저 처리 (또는 루프 시작 전 대기)
  try {
    const elevationPoints = await elevationTask;

    /**
     * [Optimization]
     * 수집된 포인트를 타일 해상도(resolution)에 맞게 확장하여 height 채널에 굽습니다.
     * 이 과정에서 Bilinear Interpolation을 적용하면 지형이 부드러워집니다.
     */
    const heightGrid = bakeElevationGrid(elevationPoints);
    currentGrids.terrain.height = heightGrid; // 🎯 지침: 고도는 전용 채널 혹은 Z축 데이터로 통합

    // 고도 데이터가 준비되면 첫 번째 점진적 업데이트 발송
    onProgress?.({
      key,
      origin,
      grids: currentGrids,
      infras: accumulatedData.infras,
      pois: accumulatedData.pois,
      resolution,
    });
  } catch (error) {
    console.warn(`[TileProcessor] Elevation step failed in ${key}.`, error);
  }

  // 2. 나머지 OSM 단계별 루프 실행
  for (let i = 0; i < steps.length; i++) {
    const step = steps[i];
    if (!step.query?.trim()) continue;

    try {
      const stepResult = await executeStep(step, origin);

      if (stepResult) {
        mergeStepResult(accumulatedData, stepResult, step.targetChannels);

        // [점진적 업데이트] 개별 단계 완료 시 마다 UI에 알림
        onProgress?.({
          key,
          origin,
          grids: currentGrids,
          infras: accumulatedData.infras,
          pois: accumulatedData.pois,
          resolution,
        });
      }
    } catch (error) {
      console.warn(
        `[TileProcessor] ${step.name} step failed in ${key}.`,
        error,
      );
      if (step.critical) throw error;
    }

    // Public API 부하 분산을 위한 랜덤 지연 (마지막 단계 제외)
    if (i < steps.length - 1) {
      await randomDelay(500, 1500);
    }
  }

  return {
    key,
    origin,
    grids: currentGrids,
    infras: accumulatedData.infras,
    pois: accumulatedData.pois,
    resolution,
  };
}

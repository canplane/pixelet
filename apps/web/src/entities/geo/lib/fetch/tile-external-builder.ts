/**
 * @file tile-external-builder.ts
 * 서버 없이 클라이언트 런타임에서 타일 데이터를 직접 생성합니다.
 * [Incremental Retry Logic] 이미 성공한 단계는 유지하고 실패한 단계만 끈질기게 추적하여 완성합니다.
 */

import { requestOverpass } from '../../api/overpass.api';
import { GEOMETRY_FILTERS } from '../../api/overpass-query/overpass-geometry-queries';
import { BOUNDARY_FILTERS } from '../../api/overpass-query/overpass-boundary-queries';
import { POI_QUERIES } from '../../api/overpass-query/overpass-poi-queries';
import { createQuery } from '../../api/overpass-utils';
import type { TileSurface, Tile, TileSurfaceKey } from '@pixelet/core/entities/geo/model/tiles';
import { getTileBbox, unprojectBbox, getTileKey } from '@pixelet/core/entities/geo/lib/utils/tile-utils';
import { createEmptyBase, createEmptyTile } from '@pixelet/core/entities/geo/lib/utils/tile-factory';
import type { Bbox, TileCoord } from '@pixelet/core/entities/geo/lib/utils/tile-utils';

import { transformOsm4326 } from '@pixelet/core/entities/geo/lib/process/osm-4326-transformer';
import { normalizeOsmFeature } from '@pixelet/core/entities/geo/lib/process/osm-feature-normalizer';
import { assembleTile } from '@pixelet/core/entities/geo/lib/bake/tile-assembler';
import { randomDelay } from '@pixelet/core/shared/lib/aync-utils';
import type { MapPoi } from '@pixelet/core/entities/geo/model/osm-feature';
import { bakeTileElevation } from '@pixelet/core/entities/geo/lib/bake/tile-elevation-baker';
import { requestElevation } from '@/entities/geo/api/elevation.api';

type StepName = 'Elevation' | 'Infrastructure' | 'Structure' | 'POI';

interface ProcessingStep {
  name: StepName;
  query: string;
  critical: boolean; // 실패 시 전체 타일 생성을 중단할지 여부
  targetChannels: TileSurfaceKey[];
}

/**
 * [Internal] 타일 BBox를 기반으로 단계별 Overpass QL 쿼리 계획을 수립합니다.
 */
function createProcessingSteps(bbox: Bbox): ProcessingStep[] {
  const { minLat, minLng, maxLat, maxLng } = unprojectBbox(bbox);
  const bboxString = `${minLat},${minLng},${maxLat},${maxLng}`;

  // 1. 지형, 구역, 도로망 쿼리 (Infrastructure)
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
  const poiFilter = POI_QUERIES.GET_RAW_FILTER ? POI_QUERIES.GET_RAW_FILTER(bboxString) : '';
  const queryPoi = createQuery(poiFilter);

  return [
    {
      name: 'Infrastructure' as StepName,
      query: queryInfra,
      critical: true,
      targetChannels: ['land', 'water', 'transport'],
    },
    {
      name: 'Structure' as StepName,
      query: queryStructure,
      critical: false,
      targetChannels: ['structure'],
    },
    {
      name: 'POI' as StepName,
      query: queryPoi,
      critical: false,
      targetChannels: [],
    },
  ];
}

/**
 * [Helper 1] 개별 단계를 실행하여 가공된 데이터를 반환합니다.
 */
async function executeStep(coord: TileCoord, step: ProcessingStep) {
  const response = await requestOverpass(step.query);
  if (!response?.elements?.length) return null;

  const elements = transformOsm4326(response.elements).map((input) => normalizeOsmFeature(input));

  return assembleTile(coord, elements, {
    limited: true,
  });
}

/**
 * [Helper 2] 가공된 데이터를 현재 타일 상태에 병합합니다.
 */
function mergeStepResult(
  targetBase: ReturnType<typeof createEmptyBase>,
  source: {
    surface: Partial<TileSurface>;
    pois: MapPoi[];
  },
  targetChannels: TileSurfaceKey[],
) {
  targetChannels.forEach((channel) => {
    const channelData = source.surface[channel];
    if (channelData) {
      targetBase.surface[channel] = channelData;
    }
  });
}

/**
 * [Main Orchestrator] Public API 모드용 타일 프로세서
 * Overpass API 호출 -> 데이터 가공 -> 점진적 업데이트 -> 최종 타일 반환의 워크플로우를 관리합니다.
 * 🚀 [Incremental Completion]: 이미 성공한 레이어는 보존하고 실패한 조각만 끈질기게 다시 받아옵니다.
 */
export async function buildExternalTile(coord: TileCoord, onProgress?: (partialTile: Tile) => void): Promise<Tile> {
  const key = getTileKey(coord);
  const bbox = getTileBbox(coord);

  const steps = createProcessingSteps(bbox);

  // 🎯 [DTO Alignment]: 서버와 동일한 baseLayer 구조체 생성
  const baseLayer = createEmptyBase();
  const pois: MapPoi[] = [];

  // 🎯 성공적으로 완료된 단계를 추적하는 상태 셋
  const completedSteps = new Set<StepName>();

  // 🚀 [Incremental Retry Loop] 최대 3회 시도하며 누락된 조각들을 채워나갑니다.
  for (let attempt = 1; attempt <= 3; attempt++) {
    // 1. 고도 데이터 처리 (미완료 시에만 실행)
    if (!completedSteps.has('Elevation')) {
      try {
        const elePoints = await requestElevation(bbox);
        const ele = bakeTileElevation(elePoints);
        baseLayer.ele = ele;

        completedSteps.add('Elevation');

        onProgress?.({
          ...createEmptyTile(coord),
          base: baseLayer,
          pois,
        });
      } catch (error) {
        console.warn(`[TileProcessor] Elevation attempt ${attempt} failed in ${key}.`, error);
      }
    }

    // 2. 나머지 OSM 단계별 루프 실행
    for (let i = 0; i < steps.length; i++) {
      const step = steps[i];
      if (!step.query?.trim() || completedSteps.has(step.name)) continue;

      try {
        const stepResult = await executeStep(coord, step);

        if (stepResult) {
          // POI 병합
          pois.push(...stepResult.pois);
          // Surface 채널 병합 (Sparse 구조 유지)
          mergeStepResult(baseLayer, stepResult, step.targetChannels);

          completedSteps.add(step.name);

          // [점진적 업데이트] 개별 단계 완료 시 마다 UI에 알림
          onProgress?.({
            ...createEmptyTile(coord),
            base: baseLayer,
            pois,
          });
        }
      } catch (error) {
        console.warn(`[TileProcessor] ${step.name} attempt ${attempt} failed in ${key}.`, error);
      }

      // Public API 부하 분산을 위한 랜덤 지연
      if (i < steps.length - 1) {
        await randomDelay(500, 1000);
      }
    }

    // 모든 조각이 모였는지 체크
    const allFinished = steps.every((s) => completedSteps.has(s.name)) && completedSteps.has('Elevation');
    if (allFinished) break;

    // 다음 재시도 전 지수 백오프 대기 (마지막 시도 제외)
    if (attempt < 3) {
      await randomDelay(2000 * attempt, 3000 * attempt);
    }
  }

  // 🛡️ [Final Guard] 필수 데이터(Infrastructure)가 끝내 누락되었다면 에러 전파하여 스케줄러가 재요청하게 함
  if (!completedSteps.has('Infrastructure')) {
    throw new Error(`[TileProcessor] Critical Infrastructure missing for ${key} after all retries.`);
  }

  return {
    ...createEmptyTile(coord),
    base: baseLayer,
    pois,
  };
}

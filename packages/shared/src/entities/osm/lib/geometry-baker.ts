/**
 * @file geometry-baker.ts
 * 정규화된 벡터 데이터를 4개의 독립적인 비트맵 레이어로 래스터화(Rasterization)합니다.
 * 🎯 모든 입력 데이터(OsmFeature)는 로컬 미터 좌표계로 전처리가 완료된 상태여야 합니다.
 */

import type { MeterPos } from '@pixelet/shared/shared/model/coords';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import type { OsmFeature } from '../model/osm-feature';
import { createEmptyLayers, type VoxelLayers } from '../model/osm-tile';
import { VOXEL_TYPES } from '@pixelet/shared/entities/voxel/model/voxels';

const { TILE_SIZE, CELL_SIZE } = OSM_CONSTANTS;

interface BakeOptions {
  tileSize?: number;
  cellSize?: number;
}

/**
 * [Voxel-Mapping] 카테고리별 기본 복셀 타입 정의
 * OsmFeature.category가 이 키에 해당하면 즉시 매핑합니다.
 */
const CATEGORY_TO_VOXEL: Record<string, number> = {
  water: VOXEL_TYPES.WATER,
  building: VOXEL_TYPES.BUILDING,
  forest: VOXEL_TYPES.GREEN_DEEP,
  park: VOXEL_TYPES.GREEN_LIGHT,
  sand: VOXEL_TYPES.SAND,
  // Road, Parking 등은 태그에 따라 세분화가 필요하여 별도 로직으로 처리
};

/**
 * [bakeGeometryGrid]
 * 정규화된 요소를 받아 4개의 독립적인 비트맵 레이어로 래스터화합니다.
 */
export function bakeGeometryGrid(
  elements: OsmFeature[],
  tileOrigin: { x: number; y: number },
  options: BakeOptions = {},
): { layers: VoxelLayers; resolution: number } {
  const { tileSize = TILE_SIZE, cellSize = CELL_SIZE } = options;

  // 타일 가로/세로당 복셀 셀 개수 (예: 4096 / 2 = 2048)
  const resolution = Math.floor(tileSize / cellSize);
  const layers: VoxelLayers = createEmptyLayers(resolution);

  // 1. 레이어 분류 및 베이킹 (OsmFeature의 전처리된 데이터를 직접 소비)
  elements.forEach((feature) => {
    const { tags, shape, computed } = feature;

    // 터널 및 고가 도로는 지면 그리드에 굽지 않음 (필요 시 로직 확장 가능)
    if (tags.tunnel === 'yes') return;

    // 2. [Type Resolution] 복셀 타입 및 타겟 레이어 결정
    // 기존의 분산된 태그 검사 로직을 resolveVoxelContext로 일원화
    const { voxelType, targetLayer } = resolveVoxelContext(feature, layers);

    if (!voxelType || !targetLayer) return;

    // 3. [Rasterization] 형상(Shape)에 따른 그리기 수행
    // 🎯 보강: Base Layer(지형)는 닫히지 않았어도(isClosed=false) 최대한 면으로 채웁니다.
    const forcePolygon = targetLayer === layers.base;

    if (shape.isClosed || forcePolygon) {
      renderPolygon(
        shape.points,
        voxelType,
        targetLayer,
        tileOrigin,
        resolution,
        cellSize,
        shape.holes,
      );
    } else {
      // 선형 데이터(도로, 강줄기 등)
      renderLinearFeature(
        shape.points,
        voxelType,
        targetLayer,
        tileOrigin,
        resolution,
        cellSize,
        computed.width,
      );
    }
  });

  return { layers, resolution };
}

/**
 * [Helper] Feature의 속성을 분석하여 그릴 복셀 타입과 타겟 레이어를 반환합니다.
 * 기존의 중복된 태그 검사 로직을 이곳으로 일원화했습니다.
 */
function resolveVoxelContext(
  feature: OsmFeature,
  layers: VoxelLayers,
): { voxelType: number | null; targetLayer: Uint8Array | null } {
  const { category, tags } = feature;

  // --- [Category 1: Water Layer] ---
  if (category === 'water' || tags.waterway || tags.natural === 'water') {
    return { voxelType: VOXEL_TYPES.WATER, targetLayer: layers.water };
  }

  // --- [Category 2: Transport Layer] ---
  if (
    category === 'road' ||
    tags.highway ||
    tags.railway ||
    category === 'parking'
  ) {
    // 주차장 처리
    if (
      category === 'parking' ||
      tags._isParking === 'true' ||
      tags.amenity === 'parking'
    ) {
      return { voxelType: VOXEL_TYPES.PARKING, targetLayer: layers.transport };
    }
    // 철도
    if (tags.railway) {
      return { voxelType: VOXEL_TYPES.RAIL, targetLayer: layers.transport };
    }
    // 보행자 전용
    if (
      ['pedestrian', 'footway', 'living_street', 'path'].includes(
        tags.highway || '',
      )
    ) {
      return {
        voxelType: VOXEL_TYPES.PEDESTRIAN,
        targetLayer: layers.transport,
      };
    }
    // 일반 도로
    return { voxelType: VOXEL_TYPES.ROAD, targetLayer: layers.transport };
  }

  // --- [Category 3: Structure Layer] ---
  if (category === 'building' || tags.building) {
    return { voxelType: VOXEL_TYPES.BUILDING, targetLayer: layers.structure };
  }

  // --- [Category 4: Base Layer] (Landuse & Natural) ---
  // 🎯 Category 매핑을 우선 시도하고, 없으면 상세 태그 분석(detectBaseVoxelType 로직 흡수)
  let baseType: number | null = CATEGORY_TO_VOXEL[category];

  if (!baseType) {
    baseType = resolveBaseVoxelTypeByTags(tags);
  }

  if (baseType) {
    return { voxelType: baseType, targetLayer: layers.base };
  }

  return { voxelType: null, targetLayer: null };
}

/**
 * [Legacy Logic Preserved]
 * 카테고리로 분류되지 않은 세부 Base Layer 타입을 태그 기반으로 결정합니다.
 * (기존 detectBaseVoxelType 함수의 로직과 주석을 그대로 계승)
 */
function resolveBaseVoxelTypeByTags(tags: any): number | null {
  // 1. 특수 시설 (최우선순위)
  if (tags._isInstitutional === 'true') return VOXEL_TYPES.INSTITUTIONAL;
  if (tags._isSports === 'true') return VOXEL_TYPES.SPORTS;

  // 🎯 2. [산/숲] 🌲 짙은 녹지 (GREEN_DEEP)
  if (
    (tags.natural &&
      [
        'wood',
        'scrub',
        'heath',
        'peak',
        'ridge',
        'cliff',
        'mountain',
        'volcano',
      ].includes(tags.natural)) ||
    tags.landuse === 'forest'
  ) {
    return VOXEL_TYPES.GREEN_DEEP;
  }

  // 🎯 3. [초지/공원] 🌿 연한 녹지 (GREEN_LIGHT)
  if (
    (tags.natural && ['grassland', 'fell', 'moor'].includes(tags.natural)) ||
    (tags.landuse &&
      ['grass', 'village_green', 'meadow', 'allotments'].includes(
        tags.landuse,
      )) ||
    (tags.leisure &&
      ['park', 'garden', 'nature_reserve'].includes(tags.leisure))
  ) {
    return VOXEL_TYPES.GREEN_LIGHT;
  }

  // 🎯 4. [거친 땅/바위] 🏜️ 모래 및 암석 (SAND)
  if (
    (tags.natural &&
      ['sand', 'shingle', 'bare_rock', 'beach', 'scree'].includes(
        tags.natural,
      )) ||
    tags.landuse === 'quarry'
  ) {
    return VOXEL_TYPES.SAND;
  }

  // 🎯 5. [인간 활동 구역] (RESIDENTIAL, COMMERCIAL, INDUSTRIAL)
  if (tags.landuse) {
    if (['residential', 'cemetery'].includes(tags.landuse))
      return VOXEL_TYPES.RESIDENTIAL;
    if (['commercial', 'retail'].includes(tags.landuse))
      return VOXEL_TYPES.COMMERCIAL;
    if (['industrial', 'construction'].includes(tags.landuse))
      return VOXEL_TYPES.INDUSTRIAL;
  }

  // 🎯 6. [스포츠/운동장]
  if (
    tags.leisure &&
    ['pitch', 'playground', 'golf_course', 'track', 'stadium'].includes(
      tags.leisure,
    )
  ) {
    return VOXEL_TYPES.SPORTS;
  }

  // 🎯 7. [마지막 수단]
  // 만약 위에서 아무것도 걸러지지 않았는데, landuse나 natural 키가 존재한다면
  // 최소한 BASE_LAND로라도 그려야 합니다.
  if (
    tags.landuse ||
    tags.natural ||
    tags.leisure ||
    tags.highway ||
    tags.building
  ) {
    return VOXEL_TYPES.BASE_LAND;
  }

  return null;
}

/**
 * [최적화] Scan-line Fill 알고리즘
 * 🚀 보강: shape.holes(Inner members) 대응 로직 추가 (Even-Odd rule 자동 적용)
 */
function renderPolygon(
  points: MeterPos[],
  value: number,
  grid: Uint8Array,
  origin: { x: number; y: number },
  res: number,
  cellSize: number,
  holes?: MeterPos[][],
) {
  if (points.length < 3) return;

  // 타일의 북쪽 끝 Y 좌표 (World Coordinate)
  const tileTopY = origin.y + res * cellSize;

  // 로컬 좌표 변환 헬퍼 (Standard Raster: Top-Down)
  const toLocal = (pts: MeterPos[]) =>
    pts.map((p) => ({
      x: (p.x - origin.x) / cellSize,
      // 🎯 [Correction] MaxY(북쪽)에서 p.y를 뺍니다. (Top-Down)
      y: (tileTopY - p.y) / cellSize,
    }));

  const outer = toLocal(points);
  const innerList = holes?.map((h) => toLocal(h)) || [];

  const minY = Math.max(0, Math.floor(Math.min(...outer.map((p) => p.y))));
  const maxY = Math.min(res - 1, Math.ceil(Math.max(...outer.map((p) => p.y))));

  for (let y = minY; y <= maxY; y++) {
    const intersections: number[] = [];

    // Outer boundary intersections
    findIntersections(outer, y, intersections);
    // 🚀 Inner holes intersections (짝수/홀수 규칙에 따라 구멍이 뚫림)
    innerList.forEach((hole) => findIntersections(hole, y, intersections));

    intersections.sort((a, b) => a - b);
    for (let i = 0; i < intersections.length; i += 2) {
      if (i + 1 >= intersections.length) break;
      const startX = Math.max(0, Math.ceil(intersections[i]));
      const endX = Math.min(res - 1, Math.floor(intersections[i + 1]));
      for (let x = startX; x <= endX; x++) {
        grid[y * res + x] = value;
      }
    }
  }
}

/** 교차점 계산 보조 함수 */
function findIntersections(
  nodes: { x: number; y: number }[],
  y: number,
  result: number[],
) {
  for (let i = 0, j = nodes.length - 1; i < nodes.length; j = i++) {
    if (
      (nodes[i].y <= y && nodes[j].y > y) ||
      (nodes[j].y <= y && nodes[i].y > y)
    ) {
      result.push(
        nodes[i].x +
          ((y - nodes[i].y) * (nodes[j].x - nodes[i].x)) /
            (nodes[j].y - nodes[i].y),
      );
    }
  }
}

/**
 * [최적화] 선형 보간 + Round Cap
 * 🚀 보강: 전처리된 computed.width를 직접 사용하여 런타임 parseFloat 제거
 */
function renderLinearFeature(
  points: MeterPos[],
  value: number,
  grid: Uint8Array,
  origin: { x: number; y: number },
  res: number,
  cellSize: number,
  meterWidth: number,
) {
  if (!points || points.length < 2) return;

  const tileTopY = origin.y + res * cellSize;

  // 두께 계산 (meter 단위 폭을 픽셀 반경으로 변환)
  const thickness = Math.max(1, Math.round((meterWidth / cellSize) * 0.5));

  const drawCircle = (cx: number, cy: number, r: number) => {
    for (let bx = -r; bx <= r; bx++) {
      for (let by = -r; by <= r; by++) {
        if (bx * bx + by * by <= r * r) {
          const tx = cx + bx,
            ty = cy + by;
          if (tx >= 0 && tx < res && ty >= 0 && ty < res) {
            grid[ty * res + tx] = value;
          }
        }
      }
    }
  };

  for (let i = 0; i < points.length - 1; i++) {
    const x1 = (points[i].x - origin.x) / cellSize;
    // 🎯 [Correction] Top-Down 방식 적용
    const y1 = (tileTopY - points[i].y) / cellSize;

    const x2 = (points[i + 1].x - origin.x) / cellSize;
    const y2 = (tileTopY - points[i + 1].y) / cellSize;

    // 시작/끝점 Round Cap
    drawCircle(Math.floor(x1 + 0.5), Math.floor(y1 + 0.5), thickness);
    if (i === points.length - 2) {
      drawCircle(Math.floor(x2 + 0.5), Math.floor(y2 + 0.5), thickness);
    }

    // 선분 보간 스텝 계산
    const steps = Math.max(
      1,
      Math.ceil(Math.max(Math.abs(x2 - x1), Math.abs(y2 - y1)) * 2),
    );

    for (let s = 0; s <= steps; s++) {
      const t = s / steps;
      const cx = Math.floor(x1 + (x2 - x1) * t + 0.5);
      const cy = Math.floor(y1 + (y2 - y1) * t + 0.5);

      // 두께만큼 채우기
      for (let bx = -thickness; bx <= thickness; bx++) {
        for (let by = -thickness; by <= thickness; by++) {
          const tx = cx + bx,
            ty = cy + by;
          if (tx >= 0 && tx < res && ty >= 0 && ty < res) {
            grid[ty * res + tx] = value;
          }
        }
      }
    }
  }
}

/**
 * @file tile-grid-baker.ts
 * 정규화된 벡터 데이터를 4개의 독립적인 비트맵 레이어로 래스터화(Rasterization)합니다.
 *
 * [Architectural Note]
 * 이 모듈은 '타일 한 장'을 굽는 가장 하위 레벨의 렌더러입니다.
 * 외부(Fetcher/Assembler)에서 어떤 좌표계(Floating Origin 등)를 쓰든 상관없이,
 * 이 모듈은 오직 '입력된 절대 좌표(Meter)'와 '타일 인덱스(Index)'만으로
 * 자신의 로컬 픽셀 좌표를 계산하여 그림을 그립니다.
 */

import type { Coord } from '@pixelet/shared/shared/model/coords';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import type { OsmExtra, OsmFeature } from '../../model/osm-feature';
import type { OsmTags } from '@pixelet/shared/entities/osm/model/osm-source';
import { VOXEL_TYPES } from '@pixelet/shared/entities/voxel/model/voxels';

// 🎯 [Correction] createEmptyLayers는 모델 정의 파일에서 가져옵니다.
import type { GridLayers } from '../../model/tiles';

// 🚀 [Refactor] 타일 절대 좌표 계산을 위해 유틸리티 활용
import {
  createEmptyLayers,
  getTileBbox,
  type TileCoord,
} from '../utils/tile-utils';

// [Optimization] 모듈 스코프 상수 캐싱 (함수 인자 전달 비용 제거)
// 렌더링 루프에서 빈번하게 사용되므로 상위 스코프에 선언하여 클로저로 접근합니다.
const { CELL_SIZE: cellSize, TILE_RESOLUTION: res } = OSM_CONSTANTS;

/**
 * [Voxel-Mapping] 카테고리별 기본 복셀 타입 정의
 * 복잡한 태그 검사 없이 category만으로 매핑 가능한 경우를 정의합니다.
 */
const CATEGORY_TO_VOXEL: Record<string, number> = {
  water: VOXEL_TYPES.WATER,
  building: VOXEL_TYPES.BUILDING,
  forest: VOXEL_TYPES.GREEN_DEEP,
  park: VOXEL_TYPES.GREEN_LIGHT,
  sand: VOXEL_TYPES.SAND,
};

/**
 * [bakeTileGrid]
 * 정규화된 요소를 받아 4개의 독립적인 비트맵 레이어로 래스터화합니다.
 *
 * @param coord - 굽고자 하는 타일의 인덱스 { tx, ty }
 * @param elements - 로컬 미터 좌표계로 전처리된 OSM Feature 목록
 */
export function bakeTileGrid(
  coord: TileCoord,
  elements: OsmFeature[],
): GridLayers {
  const layers: GridLayers = createEmptyLayers();

  // 1. [Coordinate Anchor Calculation]
  // 타일의 '절대 시작 위치(World Origin)'를 계산합니다.
  // 이 값은 Feature의 절대 좌표(World Coordinate)를 타일 내부의 상대 좌표(0~Resolution)로
  // 변환(Projection)할 때 기준점(Anchor)이 됩니다.
  const bbox = getTileBbox(coord);
  const tileOrigin = { x: bbox.minX, y: bbox.minY };

  // 2. [Feature Iteration]
  elements.forEach((feature) => {
    const { tags, shape, computed } = feature;

    // [Filter] 터널 및 고가 도로는 지면 그리드 렌더링에서 제외
    if (tags.tunnel === 'yes') return;

    // 3. [Type Resolution] 어떤 레이어에 무슨 색(Voxel)으로 칠할지 결정
    const { voxelType, targetLayer } = resolveVoxelContext(feature, layers);

    if (!voxelType || !targetLayer) return;

    // 4. [Rasterization] 형상에 따른 그리기 수행
    // [Policy] Base Layer(지형)는 선형 데이터라도 면(Polygon)으로 강제 변환하여 빈틈을 메꿉니다.
    const forcePolygon = targetLayer === layers.base;

    if (shape.isClosed || forcePolygon) {
      renderPolygon(
        shape.points,
        voxelType,
        targetLayer,
        tileOrigin,
        shape.holes,
      );
    } else {
      renderLinearFeature(
        shape.points,
        voxelType,
        targetLayer,
        tileOrigin,
        computed.width,
      );
    }
  });

  return layers;
}

/**
 * [Helper] Feature의 속성을 분석하여 그릴 복셀 타입과 타겟 레이어를 반환합니다.
 * 기존의 분산된 태그 검사 로직을 이곳으로 일원화하여 가독성을 높였습니다.
 */
function resolveVoxelContext(
  feature: OsmFeature,
  layers: GridLayers,
): { voxelType: number | null; targetLayer: Uint8Array | null } {
  const { category, tags, extra } = feature;

  // --- [Layer 1: Water] ---
  if (category === 'water' || tags.waterway || tags.natural === 'water') {
    return { voxelType: VOXEL_TYPES.WATER, targetLayer: layers.water };
  }

  // --- [Layer 2: Transport] ---
  if (
    category === 'road' ||
    tags.highway ||
    tags.railway ||
    category === 'parking'
  ) {
    // 🚀 [Logic] Extra Flag를 우선 확인하여 태그 파싱 비용 절감
    if (extra.isParking) {
      return { voxelType: VOXEL_TYPES.PARKING, targetLayer: layers.transport };
    }
    if (tags.railway) {
      return { voxelType: VOXEL_TYPES.RAIL, targetLayer: layers.transport };
    }
    // 보행자 도로 확인
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
    // 일반 도로 (Default)
    return { voxelType: VOXEL_TYPES.ROAD, targetLayer: layers.transport };
  }

  // --- [Layer 3: Structure] ---
  if (category === 'building' || tags.building) {
    return { voxelType: VOXEL_TYPES.BUILDING, targetLayer: layers.structure };
  }

  // --- [Layer 4: Base Land] ---
  // 1차: Category 매핑 시도
  let baseType: number | null = CATEGORY_TO_VOXEL[category];

  // 2차: 매핑 실패 시 상세 태그 분석 (Legacy Logic)
  if (!baseType) {
    baseType = resolveBaseVoxelType(tags, extra);
  }

  if (baseType) {
    return { voxelType: baseType, targetLayer: layers.base };
  }

  return { voxelType: null, targetLayer: null };
}

/**
 * [Legacy Logic Preserved]
 * 카테고리로 분류되지 않은 세부 Base Layer 타입을 결정합니다.
 * 🚀 Refactor: 태그 목록 누락 없이 완벽하게 이전 로직을 보존했습니다.
 */
function resolveBaseVoxelType(tags: OsmTags, extra: OsmExtra): number | null {
  // 1. [Institutional/Sports] (Extra Flag Prioritized)
  if (extra.isInstitutional) return VOXEL_TYPES.INSTITUTIONAL;
  if (extra.isSports) return VOXEL_TYPES.SPORTS;

  // 2. [Forest/Deep Green] 산, 숲
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

  // 3. [Grass/Light Green] 초지, 공원
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

  // 4. [Sand/Rock] 거친 땅
  if (
    (tags.natural &&
      ['sand', 'shingle', 'bare_rock', 'beach', 'scree'].includes(
        tags.natural,
      )) ||
    tags.landuse === 'quarry'
  ) {
    return VOXEL_TYPES.SAND;
  }

  // 5. [Human Activity] 주거/상업/공업
  if (tags.landuse) {
    if (['residential', 'cemetery'].includes(tags.landuse))
      return VOXEL_TYPES.RESIDENTIAL;
    if (['commercial', 'retail'].includes(tags.landuse))
      return VOXEL_TYPES.COMMERCIAL;
    if (['industrial', 'construction'].includes(tags.landuse))
      return VOXEL_TYPES.INDUSTRIAL;
  }

  // 6. [Sports Fallback] (Extra Flag에서 걸러지지 않은 경우를 위한 안전망)
  if (
    tags.leisure &&
    ['pitch', 'playground', 'golf_course', 'track', 'stadium'].includes(
      tags.leisure,
    )
  ) {
    return VOXEL_TYPES.SPORTS;
  }

  // 7. [General Fallback] 기본 땅
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
 * [Rasterization: Polygon] (Scan-line Fill Algorithm)
 * 좌표계를 변환(World -> Local)하고 다각형을 픽셀 그리드에 채웁니다.
 */
function renderPolygon(
  points: Coord[],
  value: number,
  grid: Uint8Array,
  origin: { x: number; y: number },
  holes?: Coord[][],
) {
  if (points.length < 3) return;

  // 상수 res, cellSize는 상위 스코프(Module Scope)에서 참조 (Closure)
  const tileTopY = origin.y + res * cellSize;

  const toLocal = (pts: Coord[]) =>
    pts.map((p) => ({
      x: (p.x - origin.x) / cellSize,
      y: (tileTopY - p.y) / cellSize,
    }));

  const outer = toLocal(points);
  const innerList = holes?.map((h) => toLocal(h)) || [];

  const minY = Math.max(0, Math.floor(Math.min(...outer.map((p) => p.y))));
  const maxY = Math.min(res - 1, Math.ceil(Math.max(...outer.map((p) => p.y))));

  for (let y = minY; y <= maxY; y++) {
    const intersections: number[] = [];
    findIntersections(outer, y, intersections);
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

/**
 * [Math Helper] 다각형 선분과 Scan-line(y)의 교차점(x)을 찾습니다.
 */
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
 * [Rasterization: Linear] (Line Interpolation + Round Cap)
 * 좌표계를 변환하고 선형 데이터를 두께감 있게 그립니다.
 */
function renderLinearFeature(
  points: Coord[],
  value: number,
  grid: Uint8Array,
  origin: { x: number; y: number },
  meterWidth: number,
) {
  if (!points || points.length < 2) return;

  // 상수 res, cellSize는 상위 스코프(Module Scope)에서 참조
  const tileTopY = origin.y + res * cellSize;
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
    const y1 = (tileTopY - points[i].y) / cellSize;
    const x2 = (points[i + 1].x - origin.x) / cellSize;
    const y2 = (tileTopY - points[i + 1].y) / cellSize;

    drawCircle(Math.floor(x1 + 0.5), Math.floor(y1 + 0.5), thickness);
    if (i === points.length - 2) {
      drawCircle(Math.floor(x2 + 0.5), Math.floor(y2 + 0.5), thickness);
    }

    const steps = Math.max(
      1,
      Math.ceil(Math.max(Math.abs(x2 - x1), Math.abs(y2 - y1)) * 2),
    );

    for (let s = 0; s <= steps; s++) {
      const t = s / steps;
      const cx = Math.floor(x1 + (x2 - x1) * t + 0.5);
      const cy = Math.floor(y1 + (y2 - y1) * t + 0.5);

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

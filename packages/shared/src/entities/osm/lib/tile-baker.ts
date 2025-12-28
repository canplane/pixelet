/**
 * @file tile-baker.ts
 * 정규화된 벡터 데이터를 4개의 독립적인 비트맵 레이어로 래스터화(Rasterization)합니다.
 * 🎯 모든 입력 데이터(NormalizedElement)는 로컬 미터 좌표계로 전처리가 완료된 상태여야 합니다.
 */

import type { MeterPos } from '@pixelet/shared/shared/model/coords';
import { OSM_CONFIG } from '@pixelet/shared/entities/osm/osm-config';
import type { NormalizedElement } from '@pixelet/shared/entities/osm/api/normalized-types';
import {
  createEmptyGrids,
  type TileGrids,
} from '@pixelet/shared/entities/osm/model/tiles';
import { VOXEL_TYPES } from '@pixelet/shared/entities/voxel/model/voxels';

const { TILE_SIZE, CELL_SIZE } = OSM_CONFIG;

interface BakeOptions {
  tileSize?: number;
  cellSize?: number;
}

/**
 * [tile-baker]
 * 정규화된 요소를 받아 4개의 독립적인 비트맵 레이어로 래스터화합니다.
 */
export function bakeTileGrid(
  elements: NormalizedElement[],
  tileOrigin: { x: number; y: number },
  options: BakeOptions = {},
): { grids: TileGrids; resolution: number } {
  const { tileSize = TILE_SIZE, cellSize = CELL_SIZE } = options;

  const resolution = Math.floor(tileSize / cellSize);
  const grids: TileGrids = createEmptyGrids(resolution);

  // 1. 레이어 분류 및 베이킹 (NormalizedElement의 전처리된 데이터를 직접 소비)
  elements.forEach((item) => {
    const { tags, type, shape, computed } = item;

    // 터널 및 고가 도로는 지면 그리드에 굽지 않음 (필요 시 로직 확장 가능)
    if (tags.tunnel === 'yes') return;

    // --- [Category 1: Water Layer] ---
    const isWater =
      tags.natural === 'water' ||
      !!tags.waterway ||
      tags.landuse === 'reservoir' ||
      tags.water === 'river';
    if (isWater) {
      if (shape.isClosed) {
        renderPolygon(
          shape.points,
          VOXEL_TYPES.WATER,
          grids.water,
          tileOrigin,
          resolution,
          cellSize,
          shape.holes,
        );
      }
      renderLinearFeature(
        shape.points,
        VOXEL_TYPES.WATER,
        grids.water,
        tileOrigin,
        resolution,
        cellSize,
        computed.width,
      );
      return;
    }

    // --- [Category 2: Transport Layer] ---
    const isRoad = !!(
      tags.highway ||
      tags.railway ||
      tags['area:highway'] ||
      tags.junction
    );
    const isParking = tags._isParking === 'true' || tags.amenity === 'parking';

    if (isParking) {
      renderPolygon(
        shape.points,
        VOXEL_TYPES.PARKING,
        grids.transport,
        tileOrigin,
        resolution,
        cellSize,
        shape.holes,
      );
      return;
    }

    if (isRoad) {
      const transType = tags.railway
        ? VOXEL_TYPES.RAIL
        : ['pedestrian', 'footway', 'living_street', 'path'].includes(
              tags.highway || '',
            )
          ? VOXEL_TYPES.PEDESTRIAN
          : VOXEL_TYPES.ROAD;

      if (shape.isClosed) {
        renderPolygon(
          shape.points,
          transType,
          grids.transport,
          tileOrigin,
          resolution,
          cellSize,
          shape.holes,
        );
      } else {
        renderLinearFeature(
          shape.points,
          transType,
          grids.transport,
          tileOrigin,
          resolution,
          cellSize,
          computed.width,
        );
      }
      return;
    }

    // --- [Category 3: Structure Layer] ---
    const isBuilding = !!(
      tags.building ||
      tags['building:part'] ||
      tags.man_made === 'pier'
    );
    if (isBuilding) {
      renderPolygon(
        shape.points,
        VOXEL_TYPES.BUILDING,
        grids.structure,
        tileOrigin,
        resolution,
        cellSize,
        shape.holes,
      );
      return;
    }

    // --- [Category 4: Base Layer] (Landuse & Natural) ---
    const baseType = detectBaseVoxelType(tags);
    if (baseType !== null) {
      // 🎯 보강: 산이나 지형 데이터는 isClosed가 false(잘린 단면)라도
      // 최대한 폴리곤으로 간주하여 채웁니다.
      renderPolygon(
        shape.points,
        baseType,
        grids.base,
        tileOrigin,
        resolution,
        cellSize,
        shape.holes,
      );
    }
  });

  return { grids, resolution };
}

/** * [최적화] Scan-line Fill 알고리즘
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

  // 로컬 좌표 변환 헬퍼
  const toLocal = (pts: MeterPos[]) =>
    pts.map((p) => ({
      x: (p.x - origin.x) / cellSize,
      y: (p.y - origin.y) / cellSize,
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

/** * [최적화] 선형 보간 + Round Cap
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
    const x1 = (points[i].x - origin.x) / cellSize,
      y1 = (points[i].y - origin.y) / cellSize;
    const x2 = (points[i + 1].x - origin.x) / cellSize,
      y2 = (points[i + 1].y - origin.y) / cellSize;

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

/** [보조] 원본 Base 레이어 판정 로직 완벽 유지 */
function detectBaseVoxelType(tags: any): number | null {
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
        'ridge',
        'plateau',
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
      ['park', 'garden', 'nature_reserve', 'recreation_ground'].includes(
        tags.leisure,
      ))
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

  return null; // 아예 데이터가 없는 경우만 null
}

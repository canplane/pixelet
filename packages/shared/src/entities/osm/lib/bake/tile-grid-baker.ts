/**
 * @file tile-grid-baker.ts
 * 정규화된 벡터 데이터를 4개의 독립적인 비트맵 레이어로 래스터화(Rasterization)합니다.
 *
 * [Architectural Note]
 * 이 모듈은 '타일 한 장'을 굽는 가장 하위 레벨의 렌더러입니다.
 * 입력된 절대 좌표(Meter)와 타일 인덱스(Index)만으로 로컬 픽셀 좌표를 계산하여 그림을 그립니다.
 */

import type { Coord } from '@pixelet/shared/shared/model/coords';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import type { OsmFeature } from '../../model/osm-feature';
import { VOXEL_TYPES } from '@pixelet/shared/entities/voxel/model/voxels';
import type { GridLayers } from '../../model/tiles';

import {
  type TileCoord,
  getTileAbsAnchor,
  createEmptyLayers,
} from '../utils/tile-utils';

import { determineVoxelConfig } from '@pixelet/shared/entities/osm/policy/osm-policy';
import type { TileBuildConfig } from '@pixelet/shared/entities/osm/lib/bake/tile-assembler';

// [Optimization] 모듈 스코프 상수 캐싱
const { CELL_SIZE: cellSize, TILE_RESOLUTION: res } = OSM_CONSTANTS;
interface TileRenderContext {
  originX: number;
  topY: number;
  cellSize: number;
  res: number;
}

/**
 * [bakeTileGrid]
 * 정규화된 요소를 받아 독립적인 비트맵 레이어로 래스터화합니다.
 */
export function bakeTileGrid(
  coord: TileCoord,
  elements: OsmFeature[],
  config?: TileBuildConfig,
): GridLayers {
  // 1. [Layer Initialization]
  const layers: GridLayers = createEmptyLayers();

  // 2. [Context Setup]
  const anchor = getTileAbsAnchor(coord);
  const ctx: TileRenderContext = {
    originX: anchor.x,
    topY: anchor.y + res * cellSize,
    cellSize,
    res,
  };

  // 3. [Feature Split]
  // 바다 데이터(육지 형상)와 나머지 지형지물 분리
  const isSea = (f: OsmFeature) =>
    f.extra.isOcean || f.category === 'sea' || f.tags.natural === 'ocean';

  const seaFeatures = elements.filter((f) => isSea(f));
  // 렌더링 순서 보정을 위한 정렬: Rank가 낮은(덜 중요한) 것부터 먼저 그리고, 높은(중요한) 것을 나중에 그려서 덮어쓰게 함
  const otherFeatures = elements
    .filter((f) => !isSea(f))
    .sort((a, b) => a.extra.rank - b.extra.rank);

  // ===========================================================================
  // 🌊 Pass 0: [Inverse Sea Rendering]
  // 해안선 데이터셋은 '육지 형상'을 가지므로, 전체를 바다로 채운 뒤 육지를 파냅니다.
  // ===========================================================================

  // 🌊  [Safety Check] External 모드에서는 바다 채우기를 건너뜁니다.
  if (!config?.limited) {
    layers.sea.fill(VOXEL_TYPES.WATER.WATER);
  }

  // (1) 배경 초기화: 타일 전체를 바다로 채움
  layers.sea.fill(VOXEL_TYPES.WATER.WATER);

  // (2) 육지 조각하기: 육지 형상(Polygon)을 0(Empty)으로 만듦
  seaFeatures.forEach((feature) => {
    renderPolygon(
      feature.shape.points,
      0, // 🎯 0 = Empty (Land Placeholder)
      layers.sea,
      ctx,
      feature.shape.holes,
    );
  });

  // ===========================================================================
  // 🏞️ Pass 1~4: 육지 데이터 렌더링 (Masking)
  // '진짜 바다' 영역(layers.sea가 WATER인 곳)에는 육지를 그리지 않도록 마스킹합니다.
  // ===========================================================================
  otherFeatures.forEach((feature) => {
    const { shape, computed, category, tags, extra } = feature;

    // 🔥 [FIX] 터널 숨김 조건 강화
    // highway 터널뿐만 아니라 man_made 터널 구조물도 확실히 스킵
    if (
      tags.tunnel === 'yes' ||
      tags.man_made === 'tunnel' ||
      tags.waterway === 'tunnel'
    ) {
      return;
    }

    const config = determineVoxelConfig(category, tags, extra);
    if (!config) return;

    const { voxelType, layerKey } = config;
    const targetLayer = layers[layerKey];
    const forcePolygon = layerKey === 'base';

    if (shape.isClosed || forcePolygon) {
      renderPolygon(
        shape.points,
        voxelType,
        targetLayer,
        ctx,
        shape.holes,
        layers.sea, // 🛡️ 바다 레이어를 마스크로 사용
      );
    } else {
      renderLinearFeature(
        shape.points,
        voxelType,
        targetLayer,
        ctx,
        computed.width,
      );
    }
  });

  return layers;
}

/**
 * [Rasterization: Polygon] (Scan-line Fill Algorithm)
 * 🚀 Mask Logic Applied
 */
function renderPolygon(
  points: Coord[],
  value: number,
  grid: Uint8Array,
  ctx: TileRenderContext,
  holes?: Coord[][],
  maskGrid?: Uint8Array, // 🛡️ Mask Layer
) {
  if (points.length < 3) return;

  const toLocal = (pts: Coord[]) =>
    pts.map((p) => ({
      x: (p.x - ctx.originX) / ctx.cellSize,
      y: (ctx.topY - p.y) / ctx.cellSize,
    }));

  const outer = toLocal(points);
  const innerList = holes?.map((h) => toLocal(h)) || [];

  const minY = Math.max(0, Math.floor(Math.min(...outer.map((p) => p.y))));
  const maxY = Math.min(
    ctx.res - 1,
    Math.ceil(Math.max(...outer.map((p) => p.y))),
  );

  for (let y = minY; y <= maxY; y++) {
    const intersections: number[] = [];
    findIntersections(outer, y, intersections);
    innerList.forEach((hole) => findIntersections(hole, y, intersections));
    intersections.sort((a, b) => a - b);

    // 🛡️ [Safety] 타일 경계 밖 시작 보정 (홀수 교차점 방어)
    if (intersections.length % 2 !== 0) {
      intersections.unshift(0);
    }

    for (let i = 0; i < intersections.length; i += 2) {
      if (i + 1 >= intersections.length) break;
      const startX = Math.max(0, Math.ceil(intersections[i]));
      const endX = Math.min(ctx.res - 1, Math.floor(intersections[i + 1]));

      for (let x = startX; x <= endX; x++) {
        const idx = y * ctx.res + x;

        // 🎯 [Mask Check] 마스크(바다)가 채워져 있다면(0이 아니라면) 그리지 않음
        // 즉, 바다 영역에는 육지 데이터를 덮어쓰지 않음
        if (maskGrid && maskGrid[idx] !== 0) continue;

        grid[idx] = value;
      }
    }
  }
}

/**
 * [Math Helper] Intersection Finder
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
 * [Rasterization: Linear]
 */
function renderLinearFeature(
  points: Coord[],
  value: number,
  grid: Uint8Array,
  ctx: TileRenderContext,
  meterWidth: number,
) {
  if (!points || points.length < 2) return;

  const thickness = Math.max(1, Math.round((meterWidth / ctx.cellSize) * 0.5));

  const drawCircle = (cx: number, cy: number, r: number) => {
    for (let bx = -r; bx <= r; bx++) {
      for (let by = -r; by <= r; by++) {
        if (bx * bx + by * by <= r * r) {
          const tx = cx + bx,
            ty = cy + by;
          if (tx >= 0 && tx < ctx.res && ty >= 0 && ty < ctx.res) {
            grid[ty * ctx.res + tx] = value;
          }
        }
      }
    }
  };

  for (let i = 0; i < points.length - 1; i++) {
    const x1 = (points[i].x - ctx.originX) / ctx.cellSize;
    const y1 = (ctx.topY - points[i].y) / ctx.cellSize;
    const x2 = (points[i + 1].x - ctx.originX) / ctx.cellSize;
    const y2 = (ctx.topY - points[i + 1].y) / ctx.cellSize;

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
          if (tx >= 0 && tx < ctx.res && ty >= 0 && ty < ctx.res) {
            grid[ty * ctx.res + tx] = value;
          }
        }
      }
    }
  }
}

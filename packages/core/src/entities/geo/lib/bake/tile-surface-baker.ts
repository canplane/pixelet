/**
 * @file tile-surface-baker.ts
 * 정규화된 벡터 데이터를 Sparse Quadtree 구조의 4개 채널로 래스터화합니다.
 *
 * [Architectural Note]
 * 1. 로컬 렌더링: 입력된 절대 좌표(Meter)를 타일 로컬 픽셀 좌표로 변환하여 래스터화합니다.
 * 2. 🎯 South-to-North Order (Critical):
 * 이 베이커는 이미지를 생성할 때 Y축의 아래쪽(0)을 남쪽(South), 위쪽(res-1)을 북쪽(North)으로
 * 매핑하여 굽습니다. 이는 프로젝트 전반의 월드 좌표계 정합성을 위함입니다.
 * 3. No-Flip Policy:
 * 생성된 바이너리는 별도의 Y-Flip 없이 클라이언트 쉐이더의 UV 좌표계와 1:1로 대응됩니다.
 * 4. 🚀 STL Adaptation: 구식 TileCells 파편을 제거하고 표준 SparseQuadtree 인터페이스를 사용합니다.
 */

import type { Coord } from '@pixelet/core/shared/model/coords';
import { GEO_CONSTANTS } from '@pixelet/core/entities/geo/model/geo-constants';
import type { OsmFeature } from '../../model/osm-feature';
import { BASE_TYPES } from '@pixelet/core/entities/cell/model/base';
import { TILE_SURFACE_KEYS, TileSurface } from '../../model/tiles';

import { type TileCoord, getTileOrigin } from '../utils/tile-utils';
import { determineCellConfig } from '@pixelet/core/entities/geo/policy/osm-policy';
import type { TileBuildConfig } from '@pixelet/core/entities/geo/lib/bake/tile-assembler';

// 🎯 [Standard Structure]: 엔진 코어의 쿼드트리와 팩토리를 주입합니다.
import { SparseQuadtree } from '@pixelet/core/shared/lib/structs/sparse-quadtree';
import { createSparseQuadtree } from '@pixelet/core/entities/geo/lib/utils/tile-factory';

const { CELL_SIZE, TILE_RES } = GEO_CONSTANTS;

interface TileRenderContext {
  tileOriginX: number;
  tileOriginY: number;
}

/**
 * [bakeTileSurface]
 * 정규화된 요소를 받아 SparseQuadtree 채널로 래스터화합니다.
 */
export function bakeTileSurface(coord: TileCoord, elements: OsmFeature[], config?: TileBuildConfig): TileSurface {
  // 1. [Initialization] 표준 팩토리를 통해 4개 채널의 쿼드트리 생성
  const channels = {} as TileSurface;
  TILE_SURFACE_KEYS.forEach((key) => {
    channels[key] = createSparseQuadtree(Uint8Array, 0);
  });

  const tileOrigin = getTileOrigin(coord);
  const ctx: TileRenderContext = { tileOriginX: tileOrigin.x, tileOriginY: tileOrigin.y };

  // 2. [Feature Split] 바다(Sea)와 일반 요소를 분리
  const isSea = (f: OsmFeature) => f.extra.isOcean || f.category === 'sea' || f.tags.natural === 'ocean';
  const seaFeatures = elements.filter(isSea);
  const otherFeatures = elements.filter((f) => !isSea(f)).sort((a, b) => a.extra.rank - b.extra.rank);

  // 🌊 Pass 0: [Inverse Sea Rendering]
  const shouldFillSea = !config?.limited;
  if (shouldFillSea) {
    // 🚀 [Standard Access]: Macro 영역에 SEA 값을 먼저 채웁니다.
    (channels.land as SparseQuadtree<Uint8Array>).macro.fill(BASE_TYPES.LAND.SEA);
    seaFeatures.forEach((f) => {
      renderPolygon(f.shape.points, BASE_TYPES.LAND.UNKNOWN, channels.land as SparseQuadtree<Uint8Array>, ctx, f.shape.holes);
    });
  }

  // 🏞️ Pass 1~4: 나머지 데이터 렌더링
  otherFeatures.forEach((feature) => {
    const { shape, category, tags, extra, computed } = feature;
    if (tags.tunnel === 'yes' || tags.man_made === 'tunnel' || tags.waterway === 'tunnel') return;

    const cellConfig = determineCellConfig(category, tags, extra);
    if (!cellConfig) return;

    const { cellType, channelKey } = cellConfig;
    const targetChannel = channels[channelKey] as SparseQuadtree<Uint8Array>;

    const isBaseOrWater = channelKey === 'land' || channelKey === 'water';
    const maskChannel = isBaseOrWater && shouldFillSea ? (channels.land as SparseQuadtree<Uint8Array>) : undefined;

    if (shape.isClosed || channelKey === 'land') {
      renderPolygon(shape.points, cellType, targetChannel, ctx, shape.holes, maskChannel, BASE_TYPES.LAND.SEA);
    } else {
      renderLinearFeature(shape.points, cellType, targetChannel, ctx, computed.width, maskChannel, BASE_TYPES.LAND.SEA);
    }
  });

  // 🧹 [Compact] 모든 채널을 최적화하여 Uniform 상태로 압축
  TILE_SURFACE_KEYS.forEach((key) => channels[key].compact());

  return channels;
}

/**
 * [Rasterization: Polygon]
 * 다각형 스캔라인 렌더링 로직
 */
function renderPolygon(
  points: Coord[],
  value: number,
  channel: SparseQuadtree<Uint8Array>,
  ctx: TileRenderContext,
  holes?: Coord[][],
  maskChannel?: SparseQuadtree<Uint8Array>,
  maskValue?: number,
) {
  if (points.length < 3) return;

  const toLocal = (pts: Coord[]) =>
    pts.map((p) => ({
      x: (p.x - ctx.tileOriginX) / CELL_SIZE,
      y: (p.y - ctx.tileOriginY) / CELL_SIZE,
    }));

  const outer = toLocal(points);
  const innerList = holes?.map(toLocal) || [];

  const minY = Math.max(0, Math.floor(Math.min(...outer.map((p) => p.y))));
  const maxY = Math.min(TILE_RES - 1, Math.ceil(Math.max(...outer.map((p) => p.y))));

  for (let y = minY; y <= maxY; y++) {
    const intersections: number[] = [];
    findIntersections(outer, y, intersections);
    innerList.forEach((hole) => findIntersections(hole, y, intersections));
    intersections.sort((a, b) => a - b);
    if (intersections.length % 2 !== 0) intersections.unshift(0);

    for (let i = 0; i < intersections.length; i += 2) {
      const startX = Math.max(0, Math.ceil(intersections[i]));
      const endX = Math.min(TILE_RES - 1, Math.floor(intersections[i + 1]));

      for (let x = startX; x <= endX; x++) {
        if (maskChannel && maskValue !== undefined && maskChannel.get(x, y) === maskValue) continue;
        channel.set(x, y, value);
      }
    }
  }
}

/**
 * [Rasterization: Linear]
 * 선형 피처 래스터화
 */
function renderLinearFeature(
  points: Coord[],
  value: number,
  channel: SparseQuadtree<Uint8Array>,
  ctx: TileRenderContext,
  meterWidth: number,
  maskChannel?: SparseQuadtree<Uint8Array>,
  maskValue?: number,
) {
  if (!points || points.length < 2) return;
  const thickness = Math.max(1, Math.round((meterWidth / CELL_SIZE) * 0.5));

  const setPixel = (tx: number, ty: number) => {
    if (tx < 0 || tx >= TILE_RES || ty < 0 || ty >= TILE_RES) return;
    if (maskChannel && maskValue !== undefined && maskChannel.get(tx, ty) === maskValue) return;
    channel.set(tx, ty, value);
  };

  const drawCircle = (cx: number, cy: number, r: number) => {
    for (let bx = -r; bx <= r; bx++) {
      for (let by = -r; by <= r; by++) {
        if (bx * bx + by * by <= r * r) setPixel(cx + bx, cy + by);
      }
    }
  };

  for (let i = 0; i < points.length - 1; i++) {
    const x1 = (points[i].x - ctx.tileOriginX) / CELL_SIZE;
    const y1 = (points[i].y - ctx.tileOriginY) / CELL_SIZE;
    const x2 = (points[i + 1].x - ctx.tileOriginX) / CELL_SIZE;
    const y2 = (points[i + 1].y - ctx.tileOriginY) / CELL_SIZE;

    drawCircle(Math.floor(x1 + 0.5), Math.floor(y1 + 0.5), thickness);
    if (i === points.length - 2) drawCircle(Math.floor(x2 + 0.5), Math.floor(y2 + 0.5), thickness);

    const steps = Math.max(1, Math.ceil(Math.max(Math.abs(x2 - x1), Math.abs(y2 - y1)) * 2));
    for (let s = 0; s <= steps; s++) {
      const t = s / steps;
      const cx = Math.floor(x1 + (x2 - x1) * t + 0.5);
      const cy = Math.floor(y1 + (y2 - y1) * t + 0.5);
      for (let bx = -thickness; bx <= thickness; bx++) {
        for (let by = -thickness; by <= thickness; by++) {
          setPixel(cx + bx, cy + by);
        }
      }
    }
  }
}

/** [Math Helper] findIntersections */
function findIntersections(nodes: { x: number; y: number }[], y: number, result: number[]) {
  for (let i = 0, j = nodes.length - 1; i < nodes.length; j = i++) {
    if ((nodes[i].y <= y && nodes[j].y > y) || (nodes[j].y <= y && nodes[i].y > y)) {
      result.push(nodes[i].x + ((y - nodes[i].y) * (nodes[j].x - nodes[i].x)) / (nodes[j].y - nodes[i].y));
    }
  }
}

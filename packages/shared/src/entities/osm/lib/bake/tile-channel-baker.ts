/**
 * @file tile-channel-baker.ts
 * 정규화된 벡터 데이터를 4개의 독립적인 비트맵 채널(Channel)로 래스터화(Rasterization)합니다.
 *
 * [Architectural Note]
 * 1. 로컬 렌더링: 입력된 절대 좌표(Meter)를 타일 로컬 픽셀 좌표로 변환하여 래스터화합니다.
 * 2. 🎯 South-to-North Order (Critical):
 * 이 베이커는 이미지를 생성할 때 Y축의 아래쪽(0)을 남쪽(South), 위쪽(res-1)을 북쪽(North)으로
 * 매핑하여 굽습니다. 이는 프로젝트 전반의 월드 좌표계 정합성을 위함입니다.
 * 3. No-Flip Policy:
 * 생성된 바이너리는 별도의 Y-Flip 없이 클라이언트 쉐이더의 UV 좌표계와 1:1로 대응됩니다.
 */

import type { Coord } from '@pixelet/shared/shared/model/coords';
import { OSM_CONSTANTS } from '@pixelet/shared/entities/osm/model/osm-constants';
import type { OsmFeature } from '../../model/osm-feature';
import { VOXEL_TYPES } from '@pixelet/shared/entities/voxel/model/voxels';
import type { TileChannels } from '../../model/tiles';

import {
  type TileCoord,
  getTileOrigin,
  createEmptyChannels,
} from '../utils/tile-utils';

import { determineVoxelConfig } from '@pixelet/shared/entities/osm/policy/osm-policy';
import type { TileBuildConfig } from '@pixelet/shared/entities/osm/lib/bake/tile-assembler';

const { CELL_SIZE, TILE_RES } = OSM_CONSTANTS;
interface TileRenderContext {
  tileOriginX: number;
  tileOriginY: number;
}

/**
 * [bakeTileChannels]
 * 정규화된 요소를 받아 독립적인 비트맵 채널로 래스터화합니다.
 */
export function bakeTileChannels(
  coord: TileCoord,
  elements: OsmFeature[],
  config?: TileBuildConfig,
): TileChannels {
  // 1. [Channel Initialization]
  const channels: TileChannels = createEmptyChannels();

  // 2. [Context Setup]
  const tileOrigin = getTileOrigin(coord);
  const ctx: TileRenderContext = {
    tileOriginX: tileOrigin.x,
    tileOriginY: tileOrigin.y,
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
  // 🌊 Pass 0: [Inverse Sea Rendering] on Water Channel
  // 해안선 데이터셋은 '육지 형상'을 가지므로, Water 채널 전체를 바다로 채운 뒤 육지를 파냅니다.
  // ===========================================================================

  // 🌊 [Safety Check] External 모드에서는 바다 채우기를 건너뜁니다.
  const shouldFillSea = !config?.limited;

  if (shouldFillSea) {
    // (1) 배경 초기화: Water 채널 전체를 바다로 채움 (통합된 WATER 채널 사용)
    channels.water.fill(VOXEL_TYPES.WATER.SEA);

    // (2) 육지 조각하기: 육지 형상(Polygon)을 0(Empty)으로 만듦
    // 결과적으로 Water 채널에는 '바다' 영역만 값이 남게 됨
    seaFeatures.forEach((feature) => {
      renderPolygon(
        feature.shape.points,
        VOXEL_TYPES.COMMON.AIR, // 🎯 0 = Empty (Land Placeholder)
        channels.water,
        ctx,
        feature.shape.holes,
      );
    });
  }

  // ===========================================================================
  // 🏞️ Pass 1~4: 육지 데이터 렌더링 (Masking Strategy)
  // '진짜 바다' 영역(channels.water가 채워진 곳)에는 육지(Base)를 그리지 않도록 마스킹합니다.
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

    const voxelConfig = determineVoxelConfig(category, tags, extra);
    if (!voxelConfig) return;

    const { voxelType, channelKey } = voxelConfig;

    // 🎯 [Channel Selection]
    // 내륙 수계(River/Lake)도 이제 'water' 채널에 그려집니다.
    // 기존 바다 데이터 위에 덧쓰거나, 빈 공간(육지)에 그려집니다.
    const targetChannel = channels[channelKey];

    // 🛡️ [Masking Logic]
    // 1. Base(지형)는 바다(SEA)가 있는 곳을 피해야 합니다.
    // 2. 내륙 수계(INLAND_WATER)도 이미 바다(SEA)인 곳에는 덧그릴 필요가 없습니다.
    // 🎯 따라서 마스크의 적용 대상을 'base'와 'water'(내륙) 둘 다로 확장하되,
    //    마스크 값은 오직 Pass 0에서 채워진 'SEA'여야 합니다.

    const isBaseOrInlandWater = channelKey === 'base' || channelKey === 'water';
    const maskChannel =
      isBaseOrInlandWater && shouldFillSea ? channels.water : undefined;

    const forcePolygon = channelKey === 'base';

    if (shape.isClosed || forcePolygon) {
      renderPolygon(
        shape.points,
        voxelType,
        targetChannel,
        ctx,
        shape.holes,
        maskChannel, // 🛡️ 바다 영역(Water Channel)을 마스크로 사용
      );
    } else {
      renderLinearFeature(
        shape.points,
        voxelType,
        targetChannel,
        ctx,
        computed.width,
        maskChannel,
      );
    }
  });

  return channels;
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
  maskGrid?: Uint8Array, // 🛡️ Mask Layer (Usually Water Channel)
) {
  if (points.length < 3) return;

  const toLocal = (pts: Coord[]) =>
    pts.map((p) => ({
      x: (p.x - ctx.tileOriginX) / CELL_SIZE,
      y: (p.y - ctx.tileOriginY) / CELL_SIZE,
    }));

  const outer = toLocal(points);
  const innerList = holes?.map((h) => toLocal(h)) || [];

  const minY = Math.max(0, Math.floor(Math.min(...outer.map((p) => p.y))));
  const maxY = Math.min(
    TILE_RES - 1,
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
      const endX = Math.min(TILE_RES - 1, Math.floor(intersections[i + 1]));

      for (let x = startX; x <= endX; x++) {
        const idx = y * TILE_RES + x;

        // 🎯 [Mask Check] 마스크(바다)가 채워져 있다면(0이 아니라면) 그리지 않음
        // 즉, 바다 영역에는 육지 데이터(Base)를 덮어쓰지 않음
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
  maskGrid?: Uint8Array, // 🛡️ 추가
) {
  if (!points || points.length < 2) return;

  const thickness = Math.max(1, Math.round((meterWidth / CELL_SIZE) * 0.5));

  const drawCircle = (cx: number, cy: number, r: number) => {
    for (let bx = -r; bx <= r; bx++) {
      for (let by = -r; by <= r; by++) {
        if (bx * bx + by * by <= r * r) {
          const tx = cx + bx,
            ty = cy + by;
          if (tx >= 0 && tx < TILE_RES && ty >= 0 && ty < TILE_RES) {
            const idx = ty * TILE_RES + tx;
            // 🛡️ Mask Check
            if (maskGrid && maskGrid[idx] !== 0) continue;
            grid[idx] = value;
          }
        }
      }
    }
  };

  for (let i = 0; i < points.length - 1; i++) {
    const x1 = (points[i].x - ctx.tileOriginX) / CELL_SIZE;
    const y1 = (points[i].y - ctx.tileOriginY) / CELL_SIZE;
    const x2 = (points[i + 1].x - ctx.tileOriginX) / CELL_SIZE;
    const y2 = (points[i + 1].y - ctx.tileOriginY) / CELL_SIZE;

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
          if (tx >= 0 && tx < TILE_RES && ty >= 0 && ty < TILE_RES) {
            const idx = ty * TILE_RES + tx; // 🎯 인덱스 계산
            // 🛡️ Mask Check
            if (maskGrid && maskGrid[idx] !== 0) continue;
            grid[idx] = value;
          }
        }
      }
    }
  }
}

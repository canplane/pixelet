/**
 * @file tile-baker.ts
 * 벡터 데이터를 4개의 독립적인 비트맵 레이어로 래스터화(Rasterization)합니다.
 */

import type { MeterPos } from '@pixelet/shared/shared/model/coords';
import { OSM_CONFIG } from '@pixelet/shared/entities/osm/osm-config';
import type { MapInfra } from '@pixelet/shared/entities/osm/model/osms';
import { ROAD_THICKNESS_MAP } from '@pixelet/shared/entities/osm/lib/osm-mapping';
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
 * 벡터 데이터를 4개의 독립적인 비트맵 레이어로 래스터화(Rasterization)합니다.
 */
export function bakeTileGrid(
  infras: MapInfra[],
  tileOrigin: { x: number; y: number },
  options: BakeOptions = {},
): { grids: TileGrids; resolution: number } {
  const { tileSize = TILE_SIZE, cellSize = CELL_SIZE } = options;

  const resolution = Math.floor(tileSize / cellSize);
  const size = resolution * resolution;

  const grids: TileGrids = createEmptyGrids(resolution);

  const layers = {
    base: [] as { infra: MapInfra; type: number }[],
    water: [] as MapInfra[],
    transport: [] as { infra: MapInfra; type: number }[],
    structure: [] as MapInfra[],
  };

  infras.forEach((infra) => {
    const { tags, type } = infra;

    // 1. Water Layer
    if (type === 'water') {
      layers.water.push(infra);
      return;
    }

    // 2. Base Layer (Landuse & 구역 정보)
    let baseType: number | null = null;

    // ✨ [우선순위 1] 파서에서 심어준 명시적 내부 플래그 확인
    if (tags._isInstitutional === 'true') baseType = VOXEL_TYPES.INSTITUTIONAL;
    else if (tags._isSports === 'true') baseType = VOXEL_TYPES.SPORTS;
    // 주차장은 Transport 채널에서 처리하기 위해 여기서는 건너뜁니다.

    // [우선순위 2] Landuse 확인
    if (baseType === null && tags.landuse) {
      if (['residential', 'cemetery', 'allotments'].includes(tags.landuse))
        baseType = VOXEL_TYPES.RESIDENTIAL;
      else if (['commercial', 'retail'].includes(tags.landuse))
        baseType = VOXEL_TYPES.COMMERCIAL;
      else if (['industrial', 'quarry', 'construction'].includes(tags.landuse))
        baseType = VOXEL_TYPES.INDUSTRIAL;
      else if (['forest'].includes(tags.landuse))
        baseType = VOXEL_TYPES.GREEN_DEEP;
      else if (['grass', 'village_green', 'meadow'].includes(tags.landuse))
        baseType = VOXEL_TYPES.GREEN_LIGHT;
    }

    // [우선순위 3] Natural 확인
    if (baseType === null && tags.natural) {
      if (['wood', 'scrub', 'heath'].includes(tags.natural))
        baseType = VOXEL_TYPES.GREEN_DEEP;
      else if (['grassland', 'fell'].includes(tags.natural))
        baseType = VOXEL_TYPES.GREEN_LIGHT;
      else if (['sand', 'shingle', 'bare_rock', 'beach'].includes(tags.natural))
        baseType = VOXEL_TYPES.SAND;
    }

    // [우선순위 4] Leisure 확인 (Legacy fallback)
    if (baseType === null && tags.leisure) {
      if (['park', 'garden', 'nature_reserve'].includes(tags.leisure))
        baseType = VOXEL_TYPES.GREEN_LIGHT;
      else if (
        ['pitch', 'playground', 'golf_course', 'track'].includes(tags.leisure)
      )
        baseType = VOXEL_TYPES.SPORTS;
    }

    if (baseType !== null) {
      layers.base.push({ infra, type: baseType });
    }

    // 3. Transport Layer (Road, Rail, Pedestrian, Parking)
    let transType: number | null = null;

    if (tags.tunnel === 'yes') return;

    // ✨ [우선순위 1] 주차장 플래그 확인
    if (tags._isParking === 'true' || tags.amenity === 'parking') {
      transType = VOXEL_TYPES.PARKING;
    }
    // 일반 도로 및 철도
    else if (type === 'road' || tags.highway) {
      if (tags.railway) {
        transType = VOXEL_TYPES.RAIL;
      } else if (
        [
          'pedestrian',
          'footway',
          'living_street',
          'path',
          'steps',
          'platform',
        ].includes(tags.highway || '')
      ) {
        transType = VOXEL_TYPES.PEDESTRIAN;
      } else {
        transType = VOXEL_TYPES.ROAD;
      }
    } else if (tags['area:highway'] === 'yes') {
      transType = VOXEL_TYPES.PEDESTRIAN;
    }

    if (transType !== null) {
      layers.transport.push({ infra, type: transType });
      return;
    }

    // 4. Structure Layer
    if (type === 'building' || tags.building || tags.man_made) {
      layers.structure.push(infra);
    }
  });

  // --- 레이어별 베이킹 (Grid Mapping) ---

  // [Pass 1] Base Grid
  layers.base.forEach(({ infra, type }) =>
    renderPolygon(
      infra.points,
      type,
      grids.base,
      tileOrigin,
      resolution,
      cellSize,
    ),
  );

  // [Pass 2] Water Grid
  layers.water.forEach((infra) => {
    if (infra.shapePath?.isClosed) {
      renderPolygon(
        infra.points,
        VOXEL_TYPES.WATER,
        grids.water,
        tileOrigin,
        resolution,
        cellSize,
      );
    }
    renderLinearFeature(
      infra,
      VOXEL_TYPES.WATER,
      grids.water,
      tileOrigin,
      resolution,
      cellSize,
    );
  });

  // [Pass 3] Transport Grid
  layers.transport.forEach(({ infra, type }) => {
    if (infra.shapePath?.isClosed || type === VOXEL_TYPES.PARKING) {
      renderPolygon(
        infra.points,
        type,
        grids.transport,
        tileOrigin,
        resolution,
        cellSize,
      );
    } else {
      renderLinearFeature(
        infra,
        type,
        grids.transport,
        tileOrigin,
        resolution,
        cellSize,
      );
    }
  });

  // [Pass 4] Structure Grid
  layers.structure.forEach((infra) => {
    renderPolygon(
      infra.points,
      VOXEL_TYPES.BUILDING,
      grids.structure,
      tileOrigin,
      resolution,
      cellSize,
    );
  });

  return { grids, resolution };
}

/** [최적화] Scan-line Fill 알고리즘 */
function renderPolygon(
  points: MeterPos[],
  value: number,
  grid: Uint8Array,
  origin: { x: number; y: number },
  res: number,
  cellSize: number,
) {
  if (points.length < 3) return;
  const local = points.map((p) => ({
    x: (p.x - origin.x) / cellSize,
    y: (p.y - origin.y) / cellSize,
  }));

  const minY = Math.max(0, Math.floor(Math.min(...local.map((p) => p.y))));
  const maxY = Math.min(res - 1, Math.ceil(Math.max(...local.map((p) => p.y))));

  for (let y = minY; y <= maxY; y++) {
    const intersections: number[] = [];
    for (let i = 0, j = local.length - 1; i < local.length; j = i++) {
      if (
        (local[i].y <= y && local[j].y > y) ||
        (local[j].y <= y && local[i].y > y)
      ) {
        intersections.push(
          local[i].x +
            ((y - local[i].y) * (local[j].x - local[i].x)) /
              (local[j].y - local[i].y),
        );
      }
    }
    intersections.sort((a, b) => a - b);
    for (let i = 0; i < intersections.length; i += 2) {
      const startX = Math.max(0, Math.ceil(intersections[i]));
      const endX = Math.min(res - 1, Math.floor(intersections[i + 1]));
      for (let x = startX; x <= endX; x++) grid[y * res + x] = value;
    }
  }
}

/** [최적화] 선형 보간 + Round Cap */
function renderLinearFeature(
  infra: MapInfra,
  value: number,
  grid: Uint8Array,
  origin: { x: number; y: number },
  res: number,
  cellSize: number,
) {
  const { points, tags } = infra;
  if (!points || points.length < 2) return;

  let thickness = 1;

  if (value === VOXEL_TYPES.ROAD || value === VOXEL_TYPES.RAIL) {
    thickness = tags._calculatedWidth
      ? Math.max(
          1,
          Math.round((parseFloat(tags._calculatedWidth) / cellSize) * 0.5),
        )
      : (ROAD_THICKNESS_MAP[tags.highway || tags.railway || ''] ?? 1);
  } else if (value === VOXEL_TYPES.WATER) {
    thickness = tags.width
      ? Math.max(1, Math.round(parseFloat(tags.width) / cellSize))
      : 1;
  } else if (value === VOXEL_TYPES.PARKING) {
    thickness = 2;
  }

  const drawCircle = (cx: number, cy: number, r: number) => {
    for (let bx = -r; bx <= r; bx++) {
      for (let by = -r; by <= r; by++) {
        if (bx * bx + by * by <= r * r) {
          const tx = cx + bx,
            ty = cy + by;
          if (tx >= 0 && tx < res && ty >= 0 && ty < res)
            grid[ty * res + tx] = value;
        }
      }
    }
  };

  for (let i = 0; i < points.length - 1; i++) {
    const x1 = (points[i].x - origin.x) / cellSize,
      y1 = (points[i].y - origin.y) / cellSize;
    const x2 = (points[i + 1].x - origin.x) / cellSize,
      y2 = (points[i + 1].y - origin.y) / cellSize;

    drawCircle(Math.floor(x1 + 0.5), Math.floor(y1 + 0.5), thickness);
    if (i === points.length - 2)
      drawCircle(Math.floor(x2 + 0.5), Math.floor(y2 + 0.5), thickness);

    const steps = Math.max(
      1,
      Math.ceil(Math.max(Math.abs(x2 - x1), Math.abs(y2 - y1)) * 2),
    );

    for (let s = 0; s <= steps; s++) {
      const t = s / steps;
      const cx = Math.floor(x1 + (x2 - x1) * t + 0.5),
        cy = Math.floor(y1 + (y2 - y1) * t + 0.5);
      for (let bx = -thickness; bx <= thickness; bx++) {
        for (let by = -thickness; by <= thickness; by++) {
          const tx = cx + bx,
            ty = cy + by;
          if (tx >= 0 && tx < res && ty >= 0 && ty < res)
            grid[ty * res + tx] = value;
        }
      }
    }
  }
}

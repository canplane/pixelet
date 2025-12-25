/**
 * @file tile-baker.ts
 */

import type { MeterPos } from '@/shared/types/map';
import { OSM_CONFIG } from '../../config/osm-config';
import type { MapInfra } from '../processing/osm-parser';

export const VOXEL_TYPES = {
  EMPTY: 0,
  WATER: 1,
  ROAD: 2,
  BUILDING: 3,
  GREEN_DEEP: 4, // 숲, 산
  GREEN_LIGHT: 5, // 공원, 잔디
  BASE_LAND: 6, // 기본 땅
  RESIDENTIAL: 7, // 주거지
  COMMERCIAL: 8, // 상업지
  INDUSTRIAL: 9, // 공업/공사현장
  SAND: 10, // 모래/암석
  PEDESTRIAN: 11, // 보행로/광장
  RAIL: 12, // 철도

  // ✨ [New Types] 세분화 추가
  INSTITUTIONAL: 13, // 학교, 병원, 대학 (캠퍼스)
  PARKING: 14, // 주차장 (아스팔트)
  SPORTS: 15, // 운동장, 골프장
} as const;

export type VoxelType = (typeof VOXEL_TYPES)[keyof typeof VOXEL_TYPES];

const ROAD_THICKNESS_MAP: Record<string, number> = {
  motorway: 4,
  trunk: 3,
  primary: 3,
  secondary: 2,
  tertiary: 2,
  residential: 1,
  service: 1,
  unclassified: 1,
  rail: 1,
};

interface BakeOptions {
  tileSize?: number;
  cellSize?: number;
}

/**
 * [TileGrids]
 * OSM 쿼리의 4단계 패스(Pass 1~4)와 정확히 1:1 대응되는 멀티 채널 그리드입니다.
 * 각 레이어는 물리적 성격에 따라 독립적으로 관리됩니다.
 */
export interface TileGrids {
  base: Uint8Array; // [Pass 1] 지형 (Landuse, Natural, Institutional)
  water: Uint8Array; // [Pass 2] 수계 (Water)
  transport: Uint8Array; // [Pass 3] 교통 (Road, Rail, Pedestrian, Parking)
  structure: Uint8Array; // [Pass 4] 구조물 (Building, Man-made)
}

/**
 * [Factory] 빈 그리드 생성기
 * 초기 렌더링 및 병합을 위해 기본값으로 채워진 TileGrids를 생성합니다.
 */
export function createEmptyGrids(resolution: number): TileGrids {
  const size = resolution * resolution;
  return {
    base: new Uint8Array(size).fill(VOXEL_TYPES.BASE_LAND),
    water: new Uint8Array(size).fill(VOXEL_TYPES.EMPTY),
    transport: new Uint8Array(size).fill(VOXEL_TYPES.EMPTY),
    structure: new Uint8Array(size).fill(VOXEL_TYPES.EMPTY),
  };
}

/**
 * [tile-baker]
 * 벡터 데이터를 4개의 독립적인 비트맵 레이어로 래스터화(Rasterization)합니다.
 * 쿼리 구조와 동일한 계층 구조를 유지하여 3D 생성 시 레이어별 제어가 가능합니다.
 */
export function bakeTileGrid(
  infras: MapInfra[],
  tileOrigin: { x: number; z: number },
  options: BakeOptions = {},
): { grids: TileGrids; resolution: number } {
  const { tileSize = OSM_CONFIG.TILE_SIZE, cellSize = OSM_CONFIG.CELL_SIZE } =
    options;

  const resolution = Math.floor(tileSize / cellSize);
  const size = resolution * resolution;

  // [핵심] 4개 레이어 초기화
  const grids: TileGrids = {
    base: new Uint8Array(size).fill(VOXEL_TYPES.BASE_LAND),
    water: new Uint8Array(size).fill(VOXEL_TYPES.EMPTY),
    transport: new Uint8Array(size).fill(VOXEL_TYPES.EMPTY),
    structure: new Uint8Array(size).fill(VOXEL_TYPES.EMPTY),
  };

  // 중간 분류용 배열 (타입 정보를 함께 저장하여 최적화)
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

    // 2. Base Layer (Landuse & Natural & Amenity)
    let baseType: number | null = null;

    // (1) Landuse 우선 확인
    if (tags.landuse) {
      if (['residential', 'cemetery', 'allotments'].includes(tags.landuse))
        baseType = VOXEL_TYPES.RESIDENTIAL;
      else if (['commercial', 'retail'].includes(tags.landuse))
        baseType = VOXEL_TYPES.COMMERCIAL;
      else if (['industrial', 'quarry', 'construction'].includes(tags.landuse))
        baseType = VOXEL_TYPES.INDUSTRIAL;
      else if (['forest'].includes(tags.landuse))
        baseType = VOXEL_TYPES.GREEN_DEEP;
      else if (['grass', 'village_green'].includes(tags.landuse))
        baseType = VOXEL_TYPES.GREEN_LIGHT;
    }

    // (2) Natural 확인
    if (!baseType && tags.natural) {
      if (['wood', 'scrub', 'heath'].includes(tags.natural))
        baseType = VOXEL_TYPES.GREEN_DEEP;
      else if (['grassland'].includes(tags.natural))
        baseType = VOXEL_TYPES.GREEN_LIGHT;
      else if (['sand', 'shingle', 'bare_rock', 'beach'].includes(tags.natural))
        baseType = VOXEL_TYPES.SAND;
    }

    // (3) Leisure 확인 (공원 및 체육시설)
    if (!baseType && tags.leisure) {
      if (['park', 'garden', 'nature_reserve'].includes(tags.leisure))
        baseType = VOXEL_TYPES.GREEN_LIGHT;
      else if (
        ['pitch', 'playground', 'golf_course', 'track'].includes(tags.leisure)
      )
        baseType = VOXEL_TYPES.SPORTS;
    }

    // (4) Amenity 확인 (학교, 병원 등 대규모 시설)
    if (!baseType && tags.amenity) {
      if (
        ['university', 'hospital', 'school', 'college'].includes(tags.amenity)
      )
        baseType = VOXEL_TYPES.INSTITUTIONAL;
    }

    if (baseType !== null) {
      layers.base.push({ infra, type: baseType });
    }

    // 3. Transport Layer (Pedestrian, Road, Rail, Parking)
    let transType: number | null = null;

    if (tags.tunnel === 'yes') return; // 터널은 렌더링 제외

    // 주차장 (Amenity)
    if (tags.amenity === 'parking') {
      transType = VOXEL_TYPES.PARKING;
    }
    // 일반 도로 및 철도
    else if (type === 'road' || tags.highway) {
      if (tags.railway) {
        transType = VOXEL_TYPES.RAIL;
      } else if (
        ['pedestrian', 'footway', 'living_street', 'path', 'steps'].includes(
          tags.highway || '',
        )
      ) {
        transType = VOXEL_TYPES.PEDESTRIAN;
      } else {
        transType = VOXEL_TYPES.ROAD;
      }
    }
    // 면(Area) 형태의 도로/보행로
    else if (tags['area:highway'] === 'yes') {
      transType = VOXEL_TYPES.PEDESTRIAN;
    }

    if (transType !== null) {
      layers.transport.push({ infra, type: transType });
      // Transport로 분류되면 Base나 Structure 처리는 건너뜀 (중복 방지)
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
    // 주차장이나 폐곡선(Area) 형태는 Polygon으로 채움
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
  origin: { x: number; z: number },
  res: number,
  cellSize: number,
) {
  if (points.length < 3) return;
  const local = points.map((p) => ({
    x: (p.x - origin.x) / cellSize,
    y: (p.z - origin.z) / cellSize,
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
  origin: { x: number; z: number },
  res: number,
  cellSize: number,
) {
  const { points, tags } = infra;
  if (!points || points.length < 2) return;

  let thickness = 1;

  // 두께 결정 로직
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
    thickness = 2; // 주차장 선형 표현 시 약간 두껍게
  }

  const drawCircle = (cx: number, cz: number, r: number) => {
    for (let bx = -r; bx <= r; bx++) {
      for (let bz = -r; bz <= r; bz++) {
        if (bx * bx + bz * bz <= r * r) {
          const tx = cx + bx,
            tz = cz + bz;
          if (tx >= 0 && tx < res && tz >= 0 && tz < res)
            grid[tz * res + tx] = value;
        }
      }
    }
  };

  for (let i = 0; i < points.length - 1; i++) {
    const x1 = (points[i].x - origin.x) / cellSize,
      z1 = (points[i].z - origin.z) / cellSize;
    const x2 = (points[i + 1].x - origin.x) / cellSize,
      z2 = (points[i + 1].z - origin.z) / cellSize;

    drawCircle(Math.floor(x1 + 0.5), Math.floor(z1 + 0.5), thickness);
    if (i === points.length - 2)
      drawCircle(Math.floor(x2 + 0.5), Math.floor(z2 + 0.5), thickness);

    const steps = Math.max(
      1,
      Math.ceil(Math.max(Math.abs(x2 - x1), Math.abs(z2 - z1)) * 2),
    );

    for (let s = 0; s <= steps; s++) {
      const t = s / steps;
      const cx = Math.floor(x1 + (x2 - x1) * t + 0.5),
        cz = Math.floor(z1 + (z2 - z1) * t + 0.5);
      for (let bx = -thickness; bx <= thickness; bx++) {
        for (let bz = -thickness; bz <= thickness; bz++) {
          const tx = cx + bx,
            tz = cz + bz;
          if (tx >= 0 && tx < res && tz >= 0 && tz < res)
            grid[tz * res + tx] = value;
        }
      }
    }
  }
}

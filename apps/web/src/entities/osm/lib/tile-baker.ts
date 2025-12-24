import { OSM_CONFIG } from '../config/osm-config';
import type { MapInfra } from './osm-parser';
import type { MeterPos } from '@/shared/types/map';

export const VOXEL_TYPES = {
  EMPTY: 0,
  WATER: 1,
  ROAD: 2,
  BUILDING: 3,
  GREEN_DEEP: 4,
  GREEN_LIGHT: 5,
  BASE_LAND: 6,
  RESIDENTIAL: 7,
  COMMERCIAL: 8,
  INDUSTRIAL: 9,
  SAND: 10,
  PEDESTRIAN: 11,
  RAIL: 12,
} as const;

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

interface TileOrigin {
  x: number;
  z: number;
}

export function bakeTileGrid(
  infras: MapInfra[],
  tileOrigin: TileOrigin,
  options: BakeOptions = {},
): { grid: Uint8Array; resolution: number } {
  const { tileSize = OSM_CONFIG.TILE_SIZE, cellSize = OSM_CONFIG.CELL_SIZE } =
    options;
  const resolution: number = Math.floor(tileSize / cellSize);
  const grid: Uint8Array = new Uint8Array(resolution * resolution).fill(
    VOXEL_TYPES.BASE_LAND,
  );

  const uniqueInfras: MapInfra[] = Array.from(
    new Map(infras.map((i: MapInfra) => [i.id, i])).values(),
  );

  const layerLanduse: { infra: MapInfra; type: number }[] = [];
  const layerNatural: { infra: MapInfra; type: number }[] = [];
  const layerWater: MapInfra[] = [];
  const layerPedestrian: MapInfra[] = [];
  const layerRoads: MapInfra[] = [];
  const layerBuildings: MapInfra[] = [];

  uniqueInfras.forEach((infra: MapInfra): void => {
    const { tags, type } = infra;

    // [추가] 수계 판정 강화
    const isWater: boolean =
      type === 'water' ||
      tags.natural === 'water' ||
      tags.waterway !== undefined ||
      tags.water === 'river' ||
      tags.water === 'basin';

    if (isWater) {
      layerWater.push(infra);
      return;
    }

    if (
      tags.landuse === 'residential' ||
      tags.landuse === 'commercial' ||
      tags.landuse === 'retail'
    ) {
      layerLanduse.push({
        infra,
        type:
          tags.landuse === 'residential'
            ? VOXEL_TYPES.RESIDENTIAL
            : VOXEL_TYPES.COMMERCIAL,
      });
    } else if (tags.landuse === 'industrial' || tags.landuse === 'quarry') {
      layerLanduse.push({ infra, type: VOXEL_TYPES.INDUSTRIAL });
    }

    if (
      tags.natural === 'wood' ||
      tags.landuse === 'forest' ||
      tags.natural === 'scrub'
    ) {
      layerNatural.push({ infra, type: VOXEL_TYPES.GREEN_DEEP });
    } else if (
      tags.leisure === 'park' ||
      tags.landuse === 'grass' ||
      tags.natural === 'grassland' ||
      tags.leisure === 'pitch'
    ) {
      layerNatural.push({ infra, type: VOXEL_TYPES.GREEN_LIGHT });
    } else if (tags.natural === 'sand' || tags.natural === 'beach') {
      layerNatural.push({ infra, type: VOXEL_TYPES.SAND });
    }

    if (
      tags.highway === 'pedestrian' ||
      tags['area:highway'] === 'yes' ||
      tags.amenity === 'parking'
    ) {
      layerPedestrian.push(infra);
    }

    if (type === 'road' || tags.highway || tags.railway) {
      layerRoads.push(infra);
    } else if (type === 'building' || tags.building) {
      layerBuildings.push(infra);
    }
  });

  // 순차 베이킹 (낮은 레이어 -> 높은 레이어)
  layerLanduse.forEach((item) =>
    bakeAreaLayer(
      [item.infra],
      grid,
      tileOrigin,
      resolution,
      cellSize,
      item.type,
      false,
    ),
  );
  layerNatural.forEach((item) =>
    bakeAreaLayer(
      [item.infra],
      grid,
      tileOrigin,
      resolution,
      cellSize,
      item.type,
      false,
    ),
  );

  // 수계: 녹지 위를 덮음
  layerWater.forEach((infra: MapInfra): void => {
    if (infra.shapePath?.isClosed) {
      bakeAreaLayer(
        [infra],
        grid,
        tileOrigin,
        resolution,
        cellSize,
        VOXEL_TYPES.WATER,
        false,
      );
    }
    bakeLinearFeature(
      infra,
      grid,
      tileOrigin,
      resolution,
      cellSize,
      VOXEL_TYPES.WATER,
    );
  });

  // 보행 구역
  layerPedestrian.forEach((infra: MapInfra): void => {
    if (infra.shapePath?.isClosed) {
      bakeAreaLayer(
        [infra],
        grid,
        tileOrigin,
        resolution,
        cellSize,
        VOXEL_TYPES.PEDESTRIAN,
        false,
      );
    } else {
      bakeLinearFeature(
        infra,
        grid,
        tileOrigin,
        resolution,
        cellSize,
        VOXEL_TYPES.PEDESTRIAN,
      );
    }
  });

  // 도로 및 철도
  layerRoads.forEach((infra: MapInfra): void => {
    if (infra.tags.tunnel === 'yes') return;
    const value: number = infra.tags.railway
      ? VOXEL_TYPES.RAIL
      : VOXEL_TYPES.ROAD;
    bakeLinearFeature(infra, grid, tileOrigin, resolution, cellSize, value);
  });

  // 건물: 가장 최상단
  bakeAreaLayer(
    layerBuildings,
    grid,
    tileOrigin,
    resolution,
    cellSize,
    VOXEL_TYPES.BUILDING,
    false,
  );

  return { grid, resolution };
}

function bakeAreaLayer(
  infras: MapInfra[],
  grid: Uint8Array,
  tileOrigin: TileOrigin,
  resolution: number,
  cellSize: number,
  value: number,
  protectExisting: boolean = false,
): void {
  const invCellSize = 1 / cellSize;
  const tileMaxX = tileOrigin.x + resolution * cellSize;
  const tileMaxZ = tileOrigin.z + resolution * cellSize;

  infras.forEach((infra: MapInfra): void => {
    if (!infra.shapePath?.isClosed) return;
    const poly: MeterPos[] = infra.points;
    const bounds = {
      minX: Math.min(...poly.map((p) => p.x)),
      maxX: Math.max(...poly.map((p) => p.x)),
      minZ: Math.min(...poly.map((p) => p.z)),
      maxZ: Math.max(...poly.map((p) => p.z)),
    };

    if (
      bounds.maxX < tileOrigin.x ||
      bounds.minX > tileMaxX ||
      bounds.maxZ < tileOrigin.z ||
      bounds.minZ > tileMaxZ
    )
      return;

    const startX = Math.max(
      0,
      Math.floor((bounds.minX - tileOrigin.x) * invCellSize),
    );
    const endX = Math.min(
      resolution - 1,
      Math.ceil((bounds.maxX - tileOrigin.x) * invCellSize),
    );
    const startZ = Math.max(
      0,
      Math.floor((bounds.minZ - tileOrigin.z) * invCellSize),
    );
    const endZ = Math.min(
      resolution - 1,
      Math.ceil((bounds.maxZ - tileOrigin.z) * invCellSize),
    );

    for (let z = startZ; z <= endZ; z++) {
      const rowOffset = z * resolution;
      const worldZ = tileOrigin.z + z * cellSize + cellSize * 0.5;
      for (let x = startX; x <= endX; x++) {
        const idx = rowOffset + x;

        // [중요] 물이나 건물은 기존 BASE_LAND가 아니어도 덮어쓸 수 있어야 함
        const isImportant =
          value === VOXEL_TYPES.WATER || value === VOXEL_TYPES.BUILDING;
        if (
          !isImportant &&
          protectExisting &&
          grid[idx] !== VOXEL_TYPES.BASE_LAND
        )
          continue;

        const worldX = tileOrigin.x + x * cellSize + cellSize * 0.5;
        if (isPointInPolygon(worldX, worldZ, poly, bounds)) {
          grid[idx] = value;
        }
      }
    }
  });
}

function bakeLinearFeature(
  infra: MapInfra,
  grid: Uint8Array,
  tileOrigin: TileOrigin,
  resolution: number,
  cellSize: number,
  value: number,
): void {
  const { points, tags } = infra;
  if (!points || points.length < 2) return;

  const invCellSize = 1 / cellSize;
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
  } else if (value === VOXEL_TYPES.PEDESTRIAN) {
    thickness = 1;
  }

  for (let i = 0; i < points.length - 1; i++) {
    const p1 = points[i];
    const p2 = points[i + 1];

    const x1 = (p1.x - tileOrigin.x) * invCellSize;
    const z1 = (p1.z - tileOrigin.z) * invCellSize;
    const x2 = (p2.x - tileOrigin.x) * invCellSize;
    const z2 = (p2.z - tileOrigin.z) * invCellSize;

    const dx = x2 - x1;
    const dz = z2 - z1;
    const distance = Math.sqrt(dx * dx + dz * dz);

    // [선분 연결 핵심] steps를 충분히 확보하여 빈틈 방지
    const steps = Math.ceil(distance * 3);

    for (let s = 0; s <= steps; s++) {
      const t = s / steps;
      const curX = Math.floor(x1 + dx * t);
      const curZ = Math.floor(z1 + dz * t);

      for (let bx = -thickness; bx <= thickness; bx++) {
        const tx = curX + bx;
        if (tx < 0 || tx >= resolution) continue;
        for (let bz = -thickness; bz <= thickness; bz++) {
          const tz = curZ + bz;
          if (tz < 0 || tz >= resolution) continue;

          const idx = tz * resolution + tx;
          // 도로는 강을 덮을 수 있고, 건물은 도로를 덮음
          grid[idx] = value;
        }
      }
    }
  }
}

function isPointInPolygon(
  x: number,
  z: number,
  poly: MeterPos[],
  bounds: any,
): boolean {
  let inside = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const xi = poly[i].x,
      zi = poly[i].z;
    const xj = poly[j].x,
      zj = poly[j].z;
    if (zi > z !== zj > z && x < ((xj - xi) * (z - zi)) / (zj - zi) + xi)
      inside = !inside;
  }
  return inside;
}

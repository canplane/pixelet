/**
 * @file tile-math.ts
 */
import { OSM_CONFIG } from '../../config/osm-config';
import type { Location, MeterPos } from '@/shared/types/map';

/**
 * 절대 좌표(MeterPos)를 받아 타일 인덱스(tx, tz)로 변환합니다.
 * 3D 좌표지만 타일은 2D 평면(XZ) 기준이므로 y값은 무시합니다.
 * * @param pos - 3D 절대 미터 좌표 {x, y, z}
 */
export const getTileIndex = (pos: MeterPos) => {
  return {
    tx: Math.floor(pos.x / OSM_CONFIG.TILE_SIZE),
    tz: Math.floor(pos.z / OSM_CONFIG.TILE_SIZE),
  };
};

/**
 * 특정 타일 인덱스의 BBox(위경도 문자열)를 반환합니다.
 * 내부적으로 3D 좌표(MeterPos)를 생성하여 unproject 함수에 전달합니다.
 * * @param tx - 타일 X 인덱스
 * @param tz - 타일 Z 인덱스
 * @param unproject - 3D 미터 좌표(MeterPos)를 위경도(Location)로 변환하는 함수
 */
export const getTileBBox = (
  tx: number,
  tz: number,
  unproject: (pos: MeterPos) => Location,
): string => {
  const { TILE_SIZE } = OSM_CONFIG;

  const minX = tx * TILE_SIZE;
  const minZ = tz * TILE_SIZE;
  const maxX = (tx + 1) * TILE_SIZE;
  const maxZ = (tz + 1) * TILE_SIZE;

  // BBox 계산 시 높이(y)는 지면 기준인 0으로 설정하여 MeterPos 인터페이스를 준수합니다.
  const sw = unproject({ x: minX, y: 0, z: minZ });
  const ne = unproject({ x: maxX, y: 0, z: maxZ });

  // Overpass Query 형식: "south,west,north,east" (minLat, minLng, maxLat, maxLng)
  const minLat = Math.min(sw.lat, ne.lat);
  const minLng = Math.min(sw.lng, ne.lng);
  const maxLat = Math.max(sw.lat, ne.lat);
  const maxLng = Math.max(sw.lng, ne.lng);

  return `${minLat},${minLng},${maxLat},${maxLng}`;
};

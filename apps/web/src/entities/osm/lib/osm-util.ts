import { OSM_CONFIG } from '../config/osm-config';

/**
 * 절대 좌표(Meter)를 타일 인덱스(tx, tz)로 변환
 */
export const getTileIndex = (x: number, z: number) => {
  return {
    tx: Math.floor(x / OSM_CONFIG.TILE_SIZE),
    tz: Math.floor(z / OSM_CONFIG.TILE_SIZE),
  };
};

/**
 * 특정 타일 인덱스의 BBox(위경도)를 반환
 */
export const getTileBBox = (
  tx: number,
  tz: number,
  unproject: (pos: { x: number; z: number }) => { lng: number; lat: number },
) => {
  const { TILE_SIZE } = OSM_CONFIG;

  const minX = tx * TILE_SIZE;
  const minZ = tz * TILE_SIZE;
  const maxX = (tx + 1) * TILE_SIZE;
  const maxZ = (tz + 1) * TILE_SIZE;

  const sw = unproject({ x: minX, z: minZ });
  const ne = unproject({ x: maxX, z: maxZ });

  // Overpass 형식: south, west, north, east
  return `${sw.lat},${sw.lng},${ne.lat},${ne.lng}`;
};

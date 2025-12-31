/** [Coord4326] EPSG:4326 (WGS84) 위경도 좌표계 */
export type Coord4326 = {
  lng: number;
  lat: number;
  alt?: number; // 고도 (단위: meter, 해수면 기준)
};

/** [Coord] EPSG:3857 (Web Mercator) 미터 좌표계 (우리 시스템 표준)
 * 지리적 절대 좌표 (단위: meter)
 * 💡 우리 프로젝트의 물리 법칙 표준입니다.
 * x: 동서 (East-West)
 * y: 남북 (North-South) -> 🎯 북쪽이 +Y
 * z: 고도 (Altitude)     -> 🎯 위쪽이 +Z
 */
export interface Coord {
  x: number;
  y: number;
  z: number;
}

/** 지구상의 위경도 및 고도 좌표 */
export type Location = {
  lng: number;
  lat: number;
  alt?: number; // 고도 (단위: meter, 해수면 기준)
};

/** * [geographic-meter-pos]
 * 지리적 절대 좌표 (단위: meter)
 * 💡 우리 프로젝트의 물리 법칙 표준입니다.
 * x: 동서 (East-West)
 * y: 남북 (North-South) -> 🎯 북쪽이 +Y
 * z: 고도 (Altitude)     -> 🎯 위쪽이 +Z
 */
export interface MeterPos {
  x: number;
  y: number;
  z: number;
}

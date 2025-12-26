/** 지구상의 위경도 및 고도 좌표 */
export type Location = {
  lng: number;
  alt?: number; // 고도 (단위: meter, 해수면 기준)
  lat: number;
};

/** 3D 월드 내 절대 좌표 (단위: meter) */
export interface MeterPos {
  x: number;
  y: number; // 3D 공간에서의 높이값
  z: number;
}

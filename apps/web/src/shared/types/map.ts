/**
 * 실제 지구의 위경도 좌표 [longitude, latitude]
 */
export type LngLat = [number, number];

/**
 * Three.js 3D 공간에서의 좌표 [x, y, z]
 * 보통 y는 높이(0)를 의미합니다.
 */
type WorldPos = [number, number, number];
export type AbsoluteWorldPos = WorldPos; // 전역 고정 원점 기준
export type RelativeWorldPos = WorldPos; // 현재 origin 기준

/**
 * 카메라 관련 데이터 (거리, 회전각 등)
 */
export interface CameraData {
  dist: number;
  rot: number;
}

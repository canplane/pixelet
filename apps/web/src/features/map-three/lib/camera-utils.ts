import { THREE_CONFIG } from '@/features/map-three/model/three-config';
import { mapConverter } from '@/shared/lib/geo/map-converter';
// import { SCALE_BAR_WIDTH } from '@/shared/ui/map/scale-bar';

/**
 * [Usage Examples - 실제 적용 시 참고]
 * * A. 오브젝트 크기 정의 (Scaling)
 * const boxSize = mapConverter.metersToUnits(10);
 * <boxGeometry args={[boxSize, boxSize, boxSize]} />
 * * B. 이동 속도 정의 (Velocity)
 * const speedPerSec = mapConverter.metersToUnits(5); // 초당 5미터 이동
 * useFrame((state, delta) => {
 * meshRef.current.position.z += speedPerSec * delta;
 * });
 * * C. 원형 가시 반경 (Radius)
 * const sensorRadius = mapConverter.metersToUnits(100); // 100m 반경 센서
 */

/**
 * 초기 카메라 위치를 계산하는 함수
 */
export const getInitialCameraPosition = (screenHeight: number) => {
  const { FOV, DEFAULT_TILT, REFERENCE_PIXELS, TARGET_METERS } =
    THREE_CONFIG.CAMERA;

  // 1. 픽셀당 실제 미터 비중 계산 (현실 세계 기준)
  const metersPerPixel = TARGET_METERS / REFERENCE_PIXELS;

  // 2. 화면 전체 높이에 해당하는 실제 미터 거리
  const visibleMeters = metersPerPixel * screenHeight;

  // 3. [수정] mapConverter를 사용하여 실제 미터를 3D 유닛으로 변환
  const visibleUnits = mapConverter.metersToUnits(visibleMeters);

  // 4. 변환된 유닛을 바탕으로 카메라 거리 계산
  const fovRad = (FOV * Math.PI) / 180;
  const distance = visibleUnits / 2 / Math.tan(fovRad / 2);

  // 5. TILT 각도 적용
  const radTilt = (DEFAULT_TILT * Math.PI) / 180;

  return {
    x: 0,
    y: distance * Math.sin(radTilt),
    z: distance * Math.cos(radTilt),
  };
};

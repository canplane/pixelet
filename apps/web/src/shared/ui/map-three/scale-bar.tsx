import { THREE_CONFIG } from '@/features/map-three/model/three-config';
import { mapConverter } from '@/shared/lib/geo/map-converter'; // 컨버터 추가

interface Props {
  distance: number; // 카메라와 타겟 사이의 거리 (unit)
}

// 축척 바(ScaleBar)의 화면상 고정 너비 (px)
const SCALE_BAR_WIDTH = THREE_CONFIG.CAMERA.REFERENCE_PIXELS;

/** 축척 바 */
export function ScaleBar({ distance }: Props) {
  const screenHeight = typeof window !== 'undefined' ? window.innerHeight : 800;

  /**
   * 축척 계산 로직:
   * 1. 현재 카메라 거리에서 화면 높이에 담기는 '3D 유닛' 가시 거리 계산
   * 2. 유닛 거리를 mapConverter를 통해 '실제 미터(m)'로 변환
   * 3. 화면 픽셀 대비 실제 미터 비율 산출
   */
  const fovRad = (THREE_CONFIG.CAMERA.FOV * Math.PI) / 180;

  // 가시 높이 (Unit 단위)
  const visibleUnits = 2 * Math.tan(fovRad / 2) * distance;

  // [수정] 유닛을 실제 미터로 변환 (SCALE_FACTOR 반영)
  const visibleMeters = mapConverter.unitsToMeters(visibleUnits);

  // 1픽셀당 실제 미터
  const metersPerPixel = visibleMeters / screenHeight;

  // UI 막대 너비가 나타내는 최종 실제 거리
  const totalMeters = SCALE_BAR_WIDTH * metersPerPixel;

  // 레이블 포맷팅
  const label =
    totalMeters >= 1000
      ? `${(totalMeters / 1000).toFixed(totalMeters >= 10000 ? 0 : 1)} km`
      : `${Math.round(totalMeters)} m`;

  return (
    <div
      style={{
        position: 'absolute',
        bottom: '40px',
        right: '40px',
        color: 'white',
        pointerEvents: 'none',
        zIndex: 100,
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        filter: 'drop-shadow(0px 0px 2px rgba(0,0,0,0.8))', // 텍스트 가독성 일괄 부여
      }}
    >
      <div
        style={{
          fontSize: '12px',
          marginBottom: '4px',
          fontWeight: 'bold',
        }}
      >
        {label}
      </div>
      <div
        style={{
          width: `${SCALE_BAR_WIDTH}px`,
          height: '6px',
          border: '2px solid white',
          borderTop: 'none',
        }}
      />
    </div>
  );
}

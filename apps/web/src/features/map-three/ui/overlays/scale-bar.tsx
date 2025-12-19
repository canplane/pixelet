import { THREE_CONFIG } from '@/features/map-three/model/three-config';
import { useMapStore } from '../../model/map-store';

// 축척 바(ScaleBar)의 화면상 고정 너비 (px)
const SCALE_BAR_WIDTH = 150;

/** * 축척 바: 현재 카메라 줌 레벨에 따라 화면 하단에 실제 거리를 표시합니다.
 */
export function ScaleBar() {
  const { distance } = useMapStore((state) => state.viewState);
  const triggerDistanceReset = useMapStore(
    (state) => state.triggerDistanceReset,
  );

  // SSR 환경 대응을 위한 기본값 설정
  const screenHeight = typeof window !== 'undefined' ? window.innerHeight : 800;

  /**
   * 축척 계산 로직 (1 Unit = 1 Meter):
   * 1. 현재 카메라 거리(distance)를 기반으로 화면 높이에 담기는 실제 '미터' 가시 거리 계산
   * 2. 화면 픽셀 대비 실제 미터 비율 산출하여 SCALE_BAR_WIDTH만큼의 거리 계산
   */
  const fovRad = (THREE_CONFIG.CAMERA.FOV * Math.PI) / 180;

  // 가시 높이 (1:1 시스템이므로 자동으로 Meter 단위가 됩니다)
  const visibleMeters = 2 * Math.tan(fovRad / 2) * distance;

  // 1픽셀당 실제 미터 비율
  const metersPerPixel = visibleMeters / screenHeight;

  // UI 막대 너비(예: 100px)가 나타내는 최종 실제 거리
  const totalMeters = SCALE_BAR_WIDTH * metersPerPixel;

  // 레이블 포맷팅 (1km 이상은 km로, 그 미만은 m로 표시)
  const label =
    totalMeters >= 1000
      ? `${(totalMeters / 1000).toFixed(totalMeters >= 10000 ? 0 : 1)} km`
      : `${Math.round(totalMeters)} m`;

  return (
    <div
      onClick={triggerDistanceReset}
      style={{
        position: 'absolute',
        bottom: '40px',
        right: '40px',
        color: 'white',
        cursor: 'pointer', // 클릭 가능 표시
        pointerEvents: 'auto', // none에서 auto로 변경하여 클릭 수신
        zIndex: 100,
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        filter: 'drop-shadow(0px 0px 3px rgba(0,0,0,0.9))',
        transition: 'transform 0.2s ease', // 호버 효과용
      }}
      // 살짝 커지는 호버 효과 추가
      onMouseEnter={(e) => (e.currentTarget.style.transform = 'scale(1.1)')}
      onMouseLeave={(e) => (e.currentTarget.style.transform = 'scale(1)')}
    >
      <div
        style={{
          fontSize: '13px',
          marginBottom: '2px',
          fontWeight: 'bold',
          fontFamily: 'monospace', // 숫자 가독성을 위해 고정폭 글꼴 권장
        }}
      >
        {label}
      </div>
      <div
        style={{
          width: `${SCALE_BAR_WIDTH}px`,
          height: '6px',
          border: '2px solid white',
          borderTop: 'none', // ㄷ자 형태의 축척 바 디자인
        }}
      />
    </div>
  );
}

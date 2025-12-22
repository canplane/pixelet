import { cn } from '@/shared/lib/utils';
import { useMapNavigation } from '../../model/use-map-navigation';
import { THREE_CONFIG } from '../../model/three-config';
import { CircularButton } from '@/shared/ui/theme/glass/circular-button';
import { MotionContainer } from '@/shared/ui/layout/motion-container';

const { DEFAULT_POLAR_ANGLE } = THREE_CONFIG.CAMERA;

export function Compass() {
  const { viewState, triggerOrientationReset } = useMapNavigation();
  const { distance, bearing, polarAngle } = viewState;

  /**
   * 🚀 임계값 최종 상향 (15도 기준)
   * 1. 최소 하한선: 0.26rad (약 15도) -> 멀리서 봐도 15도 이내면 '정렬됨' 판정
   * 2. 근거리 보정: 10m일 때 약 0.5rad (약 28도)까지 대폭 허용
   */
  const tiltThreshold = Math.max(0.26, 10 / (distance + 30) + 0.1);

  /**
   * 🚀 방향(Bearing) 임계값
   * - 500m 이상일 때: 약 3도 허용
   * - 10m 근처일 때: 약 8도 허용
   */
  const bearingThreshold = Math.max(3.0, 100 / (distance + 10));

  const isRotated = Math.abs(bearing) > bearingThreshold;
  const isTilted = Math.abs(polarAngle - DEFAULT_POLAR_ANGLE) > tiltThreshold;

  const isDefaultOrientation = !isRotated && !isTilted;

  const handleClick = async () => {
    if (!isDefaultOrientation) {
      // 1단계: 방향 및 틸트 초기화
      triggerOrientationReset();
    }
  };

  return (
    <MotionContainer show={!isDefaultOrientation} variant="compass" delay={0.4}>
      <CircularButton onClick={handleClick} isActive={isDefaultOrientation}>
        {/* 🧭 N 표시: 방향과 틸트가 모두 완벽히 리셋되었을 때만 빨간색 활성화 */}
        <span
          className={cn(
            'absolute top-1 text-[9px] font-medium tracking-tighter select-none z-20 transition-colors duration-300',
            isDefaultOrientation // 🚀 추출한 변수를 사용하여 조건문 단순화
              ? 'text-red-500 shadow-[0_0_8px_rgba(239,68,68,0.4)]'
              : 'text-white/40',
          )}
        >
          N
        </span>

        {/* 🧭 바늘 회전 */}
        <div
          className="relative w-[2px] h-8 flex flex-col z-10 transition-transform duration-200 ease-[cubic-bezier(0.1,0,0,1)]"
          style={{ transform: `rotate(${-bearing}deg)` }}
        >
          <div className="flex-1 bg-red-500/80 rounded-t-full shadow-[0_0_12px_rgba(239,68,68,0.4)]" />
          <div className="flex-1 bg-white/20 rounded-b-full shadow-[0_0_8px_rgba(255,255,255,0.1)]" />
        </div>
      </CircularButton>
    </MotionContainer>
  );
}

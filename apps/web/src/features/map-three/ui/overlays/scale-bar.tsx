import { THREE_CONFIG } from '@/entities/osm/config/three-config';
import { useMapNavigation } from '@/entities/osm/model/use-map-navigation';
import { cn } from '@/shared/lib/utils';
import { MotionContainer } from '@/shared/ui/layout/motion-container';

const { DEFAULT_DIST } = THREE_CONFIG.CAMERA;

const SCALE_BAR_WIDTH = 80;

export function ScaleBar() {
  const { viewState, triggerDistanceReset } = useMapNavigation();
  const { distance } = viewState;

  const screenHeight = typeof window !== 'undefined' ? window.innerHeight : 800;
  const fovRad = (THREE_CONFIG.CAMERA.FOV * Math.PI) / 180;
  const visibleMeters = 2 * Math.tan(fovRad / 2) * distance;
  const metersPerPixel = visibleMeters / screenHeight;
  const totalMeters = SCALE_BAR_WIDTH * metersPerPixel;

  // 🚀 현재 줌이 기본값인지 체크 (오차범위 1m)
  const isDefaultZoom = Math.abs(distance - DEFAULT_DIST) < 1;

  const label =
    totalMeters >= 1000
      ? `${(totalMeters / 1000).toFixed(totalMeters >= 10000 ? 0 : 1)}km`
      : `${Math.round(totalMeters)}m`;

  return (
    <MotionContainer show={!isDefaultZoom} variant="drop" delay={0.2}>
      <div
        onClick={triggerDistanceReset}
        className="group flex flex-col items-center cursor-pointer transition-all duration-300 active:scale-95 py-2"
        title="줌 리셋"
      >
        {/* 1. 축척 텍스트: 기본 줌일 때 색상 강조 */}
        <span
          className={cn(
            'text-[8px] font-medium mb-2 tracking-[0.1em] uppercase transition-colors',
            isDefaultZoom
              ? 'text-blue-400/80'
              : 'text-white/40 group-hover:text-white/80',
          )}
        >
          {label}
        </span>

        {/* 2. 축척 막대 */}
        <div
          className="relative flex items-center"
          style={{ width: `${SCALE_BAR_WIDTH}px` }}
        >
          {/* 베이스 수평선 */}
          <div
            className={cn(
              'h-[0.5px] w-full relative transition-colors',
              isDefaultZoom ? 'bg-blue-400/30' : 'bg-white/20',
            )}
          >
            {/* 기본 줌일 때 은은한 블루 글로우 효과 */}
            {isDefaultZoom && (
              <div className="absolute inset-0 bg-gradient-to-r from-transparent via-blue-400/40 to-transparent blur-[0.5px]" />
            )}
          </div>

          {/* 왼쪽 눈금 */}
          <div
            className={cn(
              'absolute left-0 h-2 w-[0.5px] transition-colors',
              isDefaultZoom
                ? 'bg-blue-400/60'
                : 'bg-white/40 group-hover:bg-white/80',
            )}
          />

          {/* 중앙 눈금 */}
          <div
            className={cn(
              'absolute left-1/2 -translate-x-1/2 w-[2px] h-[2px] rounded-full transition-colors',
              isDefaultZoom
                ? 'bg-blue-400/40'
                : 'bg-white/20 group-hover:bg-white/50',
            )}
          />

          {/* 오른쪽 눈금 */}
          <div
            className={cn(
              'absolute right-0 h-2 w-[0.5px] transition-colors',
              isDefaultZoom
                ? 'bg-blue-400/60'
                : 'bg-white/40 group-hover:bg-white/80',
            )}
          />
        </div>
      </div>
    </MotionContainer>
  );
}

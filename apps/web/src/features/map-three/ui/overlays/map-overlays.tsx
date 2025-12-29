import { cn } from '@/shared/lib/utils';
import { Compass } from './compass';
import { ScaleBar } from './scale-bar';
import { WorldInspector } from './world-inspector';
import { LocationControl } from './location-control';
import { useMapNavigation } from '../../model/use-map-navigation';
import { PoiDetailPanel } from './poi-detail-panel';
import { MapSearchBar } from './map-search-bar';
import { Badge } from '@/shared/ui/theme/glass/badge';

/**
 * [map-overlays]
 * 3D 캔버스 위에 렌더링되는 모든 2D UI 컴포넌트들을 통합 관리합니다.
 * 지도 조작 상태(isInteracting)에 따라 UI의 시각적 피드백(Blur, Scale, Opacity)을 동적으로 제어합니다.
 */
export function MapOverlays() {
  const { setCenter, isInteracting } = useMapNavigation();

  /**
   * [get-interaction-style]
   * 사용자가 지도를 조작 중일 때 UI가 시야를 방해하지 않도록 감쇠 효과를 적용합니다.
   * 조작 중에는 pointer-events-none을 통해 클릭 간섭을 차단합니다.
   */
  const interactionStyle = cn(
    'transition-all duration-500 ease-in-out',
    isInteracting
      ? 'pointer-events-none opacity-40 scale-[0.98] blur-[0.5px]'
      : 'opacity-100 scale-100 blur-0',
  );

  return (
    <>
      {/* 1. 상단 중앙: 위치 검색바 */}
      <div className="absolute top-8 left-1/2 -translate-x-1/2 z-[100] w-[calc(100%-4rem)] flex justify-center pointer-events-none">
        <MapSearchBar className={cn(interactionStyle, 'pointer-events-auto')} />
      </div>

      {/* 2. 상단 좌측: Voxel 데이터 모니터링 패널 */}
      <div
        className={cn(
          'absolute top-8 left-8 z-50 w-64 pointer-events-none',
          interactionStyle,
        )}
      >
        <div className="pointer-events-auto">
          <WorldInspector />
        </div>
      </div>

      {/* 3. 상단 우측: 선택된 POI 정보 패널 */}
      <div
        className={cn(
          'absolute top-8 right-8 z-50 w-64 pointer-events-none',
          interactionStyle,
        )}
      >
        <div className="pointer-events-auto">
          <PoiDetailPanel />
        </div>
      </div>

      {/* 4. 하단 좌측: 원점 이동 디버그 유틸리티 */}
      <div className={cn('absolute bottom-8 left-8 z-50', interactionStyle)}>
        <Badge
          variant="glass"
          className="pointer-events-auto cursor-pointer px-4 py-1.5 bg-white/5 border-white/10 text-[9px] font-bold tracking-widest text-white/30 hover:text-white/80 transition-all rounded-full"
          /**
           * 🎯 [debug-coordinate-fix]
           * 기존: { x: 500, y: 0, z: 500 } (Y를 고도로 사용)
           * 수정: { x: 500, y: 500, z: 0 } (Y는 북쪽, Z가 고도)
           * 이제 이 버튼을 누르면 동쪽으로 500m, 북쪽으로 500m 떨어진 지점으로 이동합니다.
           */
          onClick={() => setCenter({ x: 500, y: 500, z: 0 })}
        >
          DEBUG: SHIFT ORIGIN
        </Badge>
      </div>

      {/* 5. 하단 우측: 지도 제어 도구 (나침반, 스케일바, 위치 컨트롤) */}
      <div
        className={cn(
          'absolute bottom-8 right-8 z-50 flex flex-col items-end gap-5 pointer-events-none',
          interactionStyle,
        )}
      >
        <div className={cn(!isInteracting && 'pointer-events-auto')}>
          <Compass />
        </div>

        <div
          className={cn(
            'flex items-end gap-6',
            !isInteracting && 'pointer-events-auto',
          )}
        >
          <div className="pb-4">
            <ScaleBar />
          </div>
          <LocationControl />
        </div>
      </div>
    </>
  );
}

import { cn } from '@/shared/lib/utils';
import { Compass } from './compass';
import { ScaleBar } from './scale-bar';
import { VoxelInspector } from './voxel-inspector';
import { LocationControl } from './location-control';
import { useMapStore } from '../../model/map-store';
import { PoiDetailPanel } from '../osm/poi-detail-panel';
import { MapSearchBar } from './map-location-header';

import { Badge } from '@/shared/ui/theme/glass/badge';

export function MapOverlays() {
  const setCenter = useMapStore((state) => state.setCenter);
  const isInteracting = useMapStore((state) => state.isInteracting);

  const interactionStyle = cn(
    'transition-all duration-500 ease-in-out',
    isInteracting
      ? 'pointer-events-none opacity-40 scale-[0.98] blur-[0.5px]'
      : 'pointer-events-auto opacity-100 scale-100 blur-0',
  );

  return (
    <>
      {/* 1. [Top-Center] 검색바 */}
      <div className="absolute top-8 left-1/2 -translate-x-1/2 z-[100] w-[calc(100%-4rem)] flex justify-center pointer-events-none">
        {/* 🚀 pointer-events-auto를 통해 검색바 영역만 다시 클릭이 가능하게 만듭니다. */}
        <MapSearchBar className={cn(interactionStyle, 'pointer-events-auto')} />
      </div>

      {/* 2. [Top-Left] Voxel Inspector (시스템 모니터링) */}
      <div
        className={cn(
          'absolute top-8 left-8 z-50 w-64 pointer-events-none',
          interactionStyle,
        )}
      >
        <div className="pointer-events-auto">
          <VoxelInspector />
        </div>
      </div>

      {/* 3. [Top-Right] POI Detail Panel (컨텐츠 상세) */}
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

      {/* 4. [Bottom-Left] 디버그 뱃지 */}
      <div className={cn('absolute bottom-8 left-8 z-50', interactionStyle)}>
        <Badge
          variant="glass"
          className="pointer-events-auto cursor-pointer px-4 py-1.5 bg-white/5 border-white/10 text-[9px] font-bold tracking-widest text-white/30 hover:text-white/80 transition-all rounded-full"
          onClick={() => setCenter({ x: 500, y: 0, z: 500 })}
        >
          DEBUG: SHIFT ORIGIN
        </Badge>
      </div>

      {/* 5. [Bottom-Right] 통합 제어 레이어 */}
      <div
        className={cn(
          'absolute bottom-8 right-8 z-50 flex flex-col items-end gap-5 pointer-events-none',
          interactionStyle,
        )}
      >
        {/* Compass와 하단 그룹 사이는 Gap 5 유지 */}
        <div className={cn(!isInteracting && 'pointer-events-auto')}>
          <Compass />
        </div>

        {/* 🚀 ScaleBar와 LocationControl 사이만 Gap 6 적용 */}
        <div
          className={cn(
            'flex items-end gap-6', // 이 부분만 6(24px)으로 확장
            !isInteracting && 'pointer-events-auto',
          )}
        >
          <div className="pb-4">
            {' '}
            {/* 스케일바 텍스트 높이를 고려한 하단 여백 */}
            <ScaleBar />
          </div>
          <LocationControl />
        </div>
      </div>
    </>
  );
}

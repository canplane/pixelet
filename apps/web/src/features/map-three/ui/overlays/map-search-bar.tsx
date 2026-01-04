import * as React from 'react';
import { Search } from 'lucide-react';
import { useMapStore } from '@/entities/osm/model/map-store';
import { cn } from '@/shared/lib/utils';
import { projection } from '@pixelet/core/shared/lib/map/projection';

const { unproject } = projection;

interface MapSearchBarProps {
  className?: string; // 🚀 외부(MapOverlays)에서 interactionStyle을 받기 위함
}

export function MapSearchBar({ className }: MapSearchBarProps) {
  const center = useMapStore((state) => state.center);

  const locationInfo = React.useMemo(() => {
    if (!center) return { title: '장소 검색', coords: '' };
    const loc = unproject(center);
    return {
      title: '경기도 수원시 영통구', // 실제 구현 시 동적으로 변경 가능
      coords: `${loc.lat.toFixed(6)}, ${loc.lng.toFixed(6)}`,
    };
  }, [center, unproject]);

  return (
    <div className={cn('w-full max-w-2xl', className)}>
      <div className="relative flex items-center h-[52px] px-8 group cursor-text">
        {/* 🚀 Apple Glass Background */}
        <div
          className={cn(
            'absolute inset-0 rounded-full transition-all duration-500 ease-out',
            'bg-white/[0.06] backdrop-blur-[32px]',
            'border border-white/10 border-t-white/20',
            'shadow-[0_12px_40px_rgba(0,0,0,0.15)]',
            'group-hover:bg-white/[0.1] group-hover:border-white/20',
          )}
        />

        <div className="relative flex items-center w-full font-sans">
          <Search
            size={16}
            strokeWidth={2.2}
            className="text-white/30 mr-5 transition-colors group-hover:text-blue-400/60"
          />

          <div className="flex items-center gap-4 overflow-hidden">
            <span className="text-[14px] font-semibold text-white/90 tracking-[-0.015em] whitespace-nowrap">
              {locationInfo.title}
            </span>

            {/* Blue Accent Dot */}
            <div
              className={cn(
                'w-[4px] h-[4px] rounded-full shrink-0 transition-all duration-500',
                'bg-white/15',
                'group-hover:bg-blue-400 group-hover:shadow-[0_0_10px_rgba(96,165,250,0.8)] group-hover:scale-125',
              )}
            />

            <span className="text-[11px] font-medium text-white/20 tracking-[0.02em] tabular-nums mt-[1px]">
              {locationInfo.coords}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}

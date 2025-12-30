import { useDeviceLocationStore } from '@/entities/device-location/model/device-location.store';
import { useMapStore } from '@/entities/osm/model/map-store';
import { useMapNavigation } from '@/entities/osm/model/use-map-navigation';
import { cn } from '@/shared/lib/utils';

/**
 * POI 카테고리에 따른 아이콘 및 컬러 테마를 결정합니다.
 */
const getCategoryStyles = (tags: any, isNight: boolean) => {
  const amenity = tags?.amenity;
  const shop = tags?.shop;
  const railway = tags?.railway;

  if (amenity === 'cafe')
    return {
      icon: '☕',
      color: isNight
        ? 'text-orange-300 bg-orange-900/40'
        : 'text-orange-700 bg-orange-100',
    };
  if (amenity === 'restaurant')
    return {
      icon: '🍴',
      color: isNight ? 'text-red-300 bg-red-900/40' : 'text-red-700 bg-red-100',
    };
  if (shop)
    return {
      icon: '🛍️',
      color: isNight
        ? 'text-blue-300 bg-blue-900/40'
        : 'text-blue-700 bg-blue-100',
    };
  if (railway === 'station')
    return {
      icon: '🚉',
      color: isNight
        ? 'text-yellow-200 bg-yellow-900/40'
        : 'text-yellow-700 bg-yellow-100',
    };

  return {
    icon: '📍',
    color: isNight
      ? 'text-slate-300 bg-slate-800'
      : 'text-gray-600 bg-gray-100',
  };
};

export const PoiList = () => {
  const { status } = useDeviceLocationStore();
  const { themeKey } = useMapNavigation();
  const tiles = useMapStore((state) => state.tiles);
  const setSelectedPoi = useMapStore((state) => state.setSelectedPoi);
  const isNight = themeKey === 'night';

  /**
   * 전역 스토어에서 통합된 POI 리스트 추출
   */
  const allPois = Array.from(tiles.values()).flatMap((tile) => tile.pois);

  /**
   * 🎯 데이터 추출 보강
   * 파싱된 poi 객체 내부나 tags 내부에 분산되어 있을 수 있는 필드들을 안전하게 병합합니다.
   */
  const elements = allPois
    .map((poi: any) => ({
      ...poi,
      // 우선순위: poi 루트 속성 > tags 내부 속성
      displayName: poi.name || poi.tags?.name || '이름 없는 장소',
      displayLat: poi.lat ?? poi.tags?.lat ?? 0,
      displayLng: poi.lng ?? poi.tags?.lng ?? 0,
      category: poi.tags?.amenity || poi.tags?.shop || 'place',
    }))
    .filter((item) => item.displayName !== '이름 없는 장소');

  const isLoading = status === 'ready' && tiles.size === 0;

  if (status !== 'ready' || isLoading) {
    return (
      <div
        className={cn(
          'p-4 text-xs rounded-xl border backdrop-blur-md animate-pulse',
          isNight
            ? 'bg-slate-900/80 border-white/10 text-slate-400'
            : 'bg-white/80 border-gray-200 text-gray-500',
        )}
      >
        {status !== 'ready'
          ? '📍 위치 권한 확인 중...'
          : '🔍 주변 장소 검색 중...'}
      </div>
    );
  }

  return (
    <div
      className={cn(
        'flex flex-col w-80 rounded-xl shadow-2xl border transition-all duration-500 overflow-hidden backdrop-blur-lg',
        isNight
          ? 'bg-slate-900/85 border-white/10 text-slate-100'
          : 'bg-white/90 border-slate-200 text-slate-900',
      )}
    >
      {/* Header */}
      <div
        className={cn(
          'px-4 py-3 border-b flex justify-between items-center',
          isNight
            ? 'border-white/5 bg-white/5'
            : 'border-gray-100 bg-gray-50/50',
        )}
      >
        <div className="flex items-center gap-2">
          <span className="relative flex h-2 w-2">
            <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-blue-400 opacity-75"></span>
            <span className="relative inline-flex rounded-full h-2 w-2 bg-blue-500"></span>
          </span>
          <h2 className="text-xs font-bold tracking-tight">
            주요 관심 지점 (POI)
          </h2>
        </div>
        <span
          className={cn(
            'text-[9px] font-mono px-1.5 py-0.5 rounded-full',
            isNight
              ? 'bg-blue-500/20 text-blue-300'
              : 'bg-blue-100 text-blue-700',
          )}
        >
          {elements.length}
        </span>
      </div>

      {/* List Body */}
      <div className="max-h-[350px] overflow-y-auto custom-scrollbar">
        {elements.length === 0 ? (
          <div className="py-12 text-center opacity-50">
            <p className="text-xl mb-1">📭</p>
            <p className="text-[11px]">주변에 탐색된 장소가 없습니다.</p>
          </div>
        ) : (
          <ul
            className={cn(
              'divide-y',
              isNight ? 'divide-white/5' : 'divide-gray-50',
            )}
          >
            {elements.map((item) => {
              const { icon, color } = getCategoryStyles(item.tags, isNight);

              return (
                <li
                  key={item.id}
                  onClick={() =>
                    setSelectedPoi({
                      ...item,
                      name: item.displayName,
                      category: item.category,
                    })
                  }
                  className={cn(
                    'p-3 transition-all cursor-pointer group active:scale-[0.98]',
                    isNight ? 'hover:bg-white/5' : 'hover:bg-blue-50/50',
                  )}
                >
                  <div className="flex items-center gap-3">
                    <div
                      className={cn(
                        'flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center text-sm transition-transform group-hover:scale-110',
                        color,
                      )}
                    >
                      {icon}
                    </div>

                    <div className="flex-1 min-w-0">
                      <p
                        className={cn(
                          'text-[13px] font-bold truncate transition-colors',
                          isNight
                            ? 'group-hover:text-blue-300'
                            : 'group-hover:text-blue-600',
                        )}
                      >
                        {item.displayName}
                      </p>
                      <div className="flex items-center gap-1.5 mt-0.5">
                        <span className="text-[9px] uppercase font-bold opacity-40">
                          {item.category}
                        </span>
                        <span className="w-0.5 h-0.5 rounded-full bg-current opacity-20" />
                        <span className="text-[9px] font-mono opacity-40">
                          {item.displayLat.toFixed(4)},{' '}
                          {item.displayLng.toFixed(4)}
                        </span>
                      </div>
                    </div>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </div>
  );
};

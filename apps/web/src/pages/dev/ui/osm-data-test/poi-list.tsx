import { useDeviceLocationStore } from '@/entities/device-location/model/device-location.store';
import { useMapFeatures } from '@/entities/osm/model/use-map-features';

/**
 * 카테고리별 간단한 이모지 및 색상 매핑
 */
const getCategoryInfo = (tags: any) => {
  if (tags.amenity === 'cafe')
    return { icon: '☕', color: 'bg-orange-100 text-orange-700' };
  if (tags.amenity === 'restaurant')
    return { icon: '🍴', color: 'bg-red-100 text-red-700' };
  if (tags.shop) return { icon: '🛍️', color: 'bg-blue-100 text-blue-700' };
  if (tags.tourism || tags.historic)
    return { icon: '🏛️', color: 'bg-purple-100 text-purple-700' };
  if (tags.railway === 'station')
    return { icon: '🚉', color: 'bg-yellow-100 text-yellow-700' };
  return { icon: '📍', color: 'bg-gray-100 text-gray-700' };
};

export const PoiList = () => {
  const { location: deviceLocation, status } = useDeviceLocationStore();

  const { data, isLoading, error } = useMapFeatures(
    'POI_MAJOR',
    deviceLocation,
  );

  if (status !== 'ready')
    return (
      <div className="p-4 text-sm text-gray-500">📍 위치 권한 확인 중...</div>
    );
  if (isLoading)
    return (
      <div className="p-4 text-sm text-gray-500 animate-pulse">
        🔍 주변 장소 검색 중...
      </div>
    );
  if (error)
    return (
      <div className="p-4 text-sm text-red-500">⚠️ 에러: {error.message}</div>
    );

  // [변경 포인트 1] 이름(tags.name)이 있는 요소만 필터링
  const elements = (data?.elements || []).filter(
    (item: any) => item.tags?.name,
  );

  return (
    <div className="flex flex-col w-80 bg-white/90 backdrop-blur-md rounded-xl shadow-xl border border-gray-200 overflow-hidden">
      {/* Header */}
      <div className="px-4 py-3 border-b border-gray-100 flex justify-between items-center bg-white/50">
        <h2 className="text-sm font-bold text-gray-800">
          주요 관심 지점 (POI)
        </h2>
        <span className="text-[10px] font-mono bg-blue-100 px-1.5 py-0.5 rounded text-blue-700">
          {elements.length}
        </span>
      </div>

      {/* List Body */}
      <div className="max-h-[350px] overflow-y-auto overflow-x-hidden">
        {elements.length === 0 ? (
          <div className="py-16 text-center">
            <p className="text-2xl mb-2">📭</p>
            <p className="text-xs text-gray-400">
              주변에 이름 있는 장소가 없습니다.
            </p>
          </div>
        ) : (
          <ul className="divide-y divide-gray-50">
            {elements.map((item: any) => {
              const { tags, lat, lon, center } = item;
              const displayLat = lat || center?.lat;
              const displayLng = lon || center?.lng;
              const { icon, color } = getCategoryInfo(tags);

              return (
                <li
                  key={item.id}
                  className="p-3 hover:bg-blue-50/50 transition-colors cursor-pointer group"
                >
                  <div className="flex items-start gap-3">
                    <div
                      className={`flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center text-sm ${color}`}
                    >
                      {icon}
                    </div>

                    <div className="flex-1 min-w-0">
                      {/* [변경 포인트 2] '이름 없는 장소' 폴백 제거 및 텍스트 스타일 강조 */}
                      <p className="text-sm font-semibold text-gray-900 truncate group-hover:text-blue-600">
                        {tags.name}
                      </p>
                      <div className="flex items-center gap-2 mt-0.5">
                        <span className="text-[10px] uppercase font-medium text-gray-400">
                          {tags.amenity || tags.shop || tags.tourism || '장소'}
                        </span>
                        <span className="text-[10px] text-gray-200">|</span>
                        <span className="text-[10px] text-gray-400 font-mono">
                          {displayLat?.toFixed(4)}, {displayLng?.toFixed(4)}
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

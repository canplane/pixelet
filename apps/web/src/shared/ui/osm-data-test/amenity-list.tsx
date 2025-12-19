// 1. 엔티티 및 상태 관리 (Entities)
import { useDeviceLocationStore } from '@/entities/device-location/model/device-location.store';
import { useMapFeatures } from '@/entities/osm/model/use-map-features';

/**
 * 내 주변 편의시설 목록 테스트 컴포넌트
 * 기기의 실시간 Location을 기반으로 OSM 데이터를 쿼리합니다.
 */
export const AmenityList = () => {
  // 1. 기기 실제 GPS 위치 (Location 객체 구조: { lng, lat })
  const { location: deviceLocation, status } = useDeviceLocationStore();

  // 2. OSM 데이터 호출 (쿼리 키와 위치 정보 전달)
  const {
    osmData: data,
    isLoading,
    error,
  } = useMapFeatures('AMENITIES', deviceLocation);

  // 3. 상태별 렌더링 처리
  if (status !== 'ready') return <p>📍 위치 권한을 확인하고 있습니다...</p>;
  if (isLoading) return <p>🔍 주변 시설 정보를 불러오는 중...</p>;
  if (error) return <p>⚠️ 에러 발생: {error.message}</p>;
  if (!data?.elements || data.elements.length === 0)
    return <p>📭 주변에 시설이 없습니다.</p>;

  return (
    <div className="p-4 bg-white/90 rounded-lg shadow-lg max-h-[400px] overflow-y-auto">
      <h2 className="text-lg font-bold mb-3">
        내 주변 편의시설 ({data.elements.length}곳)
      </h2>
      <ul className="space-y-2 text-sm">
        {data.elements.map((item, index) => {
          const { tags, lat, lon } = item;

          // Overpass 노드는 lat, lon이 직접 포함되며,
          // geometry 옵션 사용 시 item.geometry[0] 등으로도 접근 가능합니다.
          const displayLat = lat?.toFixed(5);
          const displayLng = lon?.toFixed(5);

          return (
            <li key={item.id} className="border-b pb-2 last:border-0">
              <div className="font-medium text-blue-600">
                [{index + 1}] {tags?.name || '이름 없음'}
              </div>
              <div className="text-gray-600">
                유형: <span className="badge">{tags?.amenity || '기타'}</span>
              </div>
              <div className="text-[10px] text-gray-400">
                위치: {displayLat}, {displayLng}
              </div>
            </li>
          );
        })}
      </ul>
    </div>
  );
};

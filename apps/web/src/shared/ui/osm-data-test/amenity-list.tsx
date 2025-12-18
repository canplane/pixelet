// import { useNearbyAmenities } from '../model/use-nearby-amenities';
import { useDeviceLocationStore } from '@/entities/device-location/model/device-location.store';
import { useMapFeatures } from '@/entities/osm/model/use-map-features';

/** Test OSM Data */
export const AmenityList = () => {
  // 1. 기기 실제 GPS 위치
  const { lngLat: deviceLngLat, status } = useDeviceLocationStore();

  // 2. API 호출
  const {
    osmData: data,
    loading,
    error,
    isWaiting,
  } = useMapFeatures('AMENITIES', deviceLngLat);

  if (loading) return <p>주변 시설 정보를 불러오는 중...</p>;
  if (error) return <p>에러 발생: {error.message}</p>;
  if (!data || !data.elements || data.elements.length === 0)
    return <p>주변에 시설이 없습니다.</p>;

  return (
    <div>
      <h2>내 주변 편의시설 ({data.elements.length}곳)</h2>
      <ul>
        {data.elements.map((item, index) => {
          // center가 있는 경우와 직접 lat/lon이 있는 경우를 모두 체크
          const pos = item.geometry ?? [item.lat, item.lon];
          //   const latitude = element.lat ?? element.center?.lat;
          //   const longitude = element.lon ?? element.center?.lon;
          const { tags } = item;

          return (
            <li key={item.id}>
              [{index}] {/* ID: {item.id} | */}
              유형: {tags?.amenity || '알 수 없음'} | 이름: {tags?.name}
              <br />
              좌표: {JSON.stringify(pos)}
              {/* 좌표: {latitude?.toFixed(4)}, {longitude?.toFixed(4)} */}
            </li>
          );
        })}
      </ul>
    </div>
  );
};

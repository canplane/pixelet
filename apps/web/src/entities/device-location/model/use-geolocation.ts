import { useEffect, useRef } from 'react';
import { useDeviceLocationStore } from '../model/device-location.store'; // 경로 확인 필요

export function useGeolocation() {
  // 스토어의 액션들을 가져옵니다.
  const { setGpsData, setError, setStatus } = useDeviceLocationStore();

  const watchId = useRef<number | null>(null);
  const retryCount = useRef(0);
  const MAX_RETRIES = 3;

  useEffect(() => {
    if (!('geolocation' in navigator)) {
      setError('이 브라우저는 위치 정보를 지원하지 않습니다.');
      return;
    }

    const startWatching = () => {
      // 로딩 상태 시작
      setStatus('loading');

      watchId.current = navigator.geolocation.watchPosition(
        (pos) => {
          const { latitude, longitude, altitude, accuracy, heading } =
            pos.coords;

          // 정확도 필터링 (3km 이상 무시)
          if (accuracy > 3000) return;

          // 전역 스토어 업데이트
          setGpsData({
            location: {
              lng: longitude,
              lat: latitude,
              alt: altitude ?? 0,
            },
            accuracy,
            heading,
          });

          retryCount.current = 0;
        },
        (err) => {
          if (err.code === err.TIMEOUT && retryCount.current < MAX_RETRIES) {
            retryCount.current++;
            return;
          }

          let friendlyMessage = '위치 정보를 가져올 수 없습니다.';
          if (err.code === err.PERMISSION_DENIED)
            friendlyMessage = '위치 권한이 거부되었습니다.';
          if (err.code === err.POSITION_UNAVAILABLE)
            friendlyMessage = '위치 신호를 잡을 수 없습니다 (GPS 확인 필요).';
          if (err.code === err.TIMEOUT)
            friendlyMessage = '위치 수신 시간이 초과되었습니다.';

          // 전역 스토어에 에러 기록
          setError(friendlyMessage);
        },
        {
          enableHighAccuracy: true,
          maximumAge: 0,
          timeout: 10000,
        },
      );
    };

    const handleVisibilityChange = () => {
      if (document.visibilityState === 'hidden') {
        if (watchId.current !== null) {
          navigator.geolocation.clearWatch(watchId.current);
          watchId.current = null;
        }
      } else {
        if (watchId.current === null) startWatching();
      }
    };

    startWatching();
    document.addEventListener('visibilitychange', handleVisibilityChange);

    return () => {
      if (watchId.current !== null)
        navigator.geolocation.clearWatch(watchId.current);
      document.removeEventListener('visibilitychange', handleVisibilityChange);
    };
  }, [setGpsData, setError, setStatus]);

  // 이제 훅은 값을 직접 리턴할 필요가 없습니다.
  // 컴포넌트들은 useDeviceLocationStore()를 통해 값을 읽으면 됩니다.
}

import { useEffect, useRef, useCallback } from 'react';
import { useDeviceLocationStore } from '../model/device-location.store';

/** * 공유 자원 관리를 위한 전역 구독자 카운트
 * 여러 컴포넌트에서 훅 호출 시 센서 중복 가동을 방지합니다.
 */
let globalGpsSubscriberCount = 0;

/**
 * [use-device-geolocation]
 * 기기 GPS 센서를 감시하여 전역 스토어와 동기화하는 싱글톤 서비스 훅입니다.
 */
export function useDeviceGeolocation() {
  const { setGpsData, setError, setStatus, status } = useDeviceLocationStore();

  const watchId = useRef<number | null>(null);
  const retryCount = useRef(0);
  const MAX_RETRIES = 3;

  /** 센서 수신 중단 및 리소스 해제 */
  const stopWatching = useCallback(() => {
    if (watchId.current !== null) {
      navigator.geolocation.clearWatch(watchId.current);
      watchId.current = null;
    }
  }, []);

  /** 센서 수신 시작 (중복 실행 가드 포함) */
  const startWatching = useCallback(() => {
    // 1. 이미 활성화된 센서가 있다면 가동하지 않음
    if (
      watchId.current !== null ||
      status === 'loading' ||
      status === 'ready'
    ) {
      return;
    }

    if (!('geolocation' in navigator)) {
      setError('이 브라우저는 위치 정보를 지원하지 않습니다.');
      return;
    }

    setStatus('loading');

    watchId.current = navigator.geolocation.watchPosition(
      (pos) => {
        const { latitude, longitude, altitude, accuracy, heading } = pos.coords;

        // 2. 정확도 필터링: 신뢰도가 현저히 낮은 데이터(3km 오차)는 폐기
        if (accuracy > 3000) return;

        setGpsData({
          location: { lng: longitude, lat: latitude, alt: altitude ?? 0 },
          accuracy,
          heading,
        });
        retryCount.current = 0;
      },
      (err) => {
        // 3. 일시적 타임아웃 발생 시 지정된 횟수만큼 재시도
        if (err.code === err.TIMEOUT && retryCount.current < MAX_RETRIES) {
          retryCount.current++;
          return;
        }

        let friendlyMessage = '위치 정보를 가져올 수 없습니다.';
        if (err.code === err.PERMISSION_DENIED)
          friendlyMessage = '위치 권한이 거부되었습니다.';
        if (err.code === err.POSITION_UNAVAILABLE)
          friendlyMessage = '위치 신호를 잡을 수 없습니다.';
        if (err.code === err.TIMEOUT)
          friendlyMessage = '수신 시간이 초과되었습니다.';

        setError(friendlyMessage);
      },
      { enableHighAccuracy: true, maximumAge: 0, timeout: 10000 },
    );
  }, [setGpsData, setError, setStatus, status]);

  /** 라이프사이클 관리 및 탭 가시성 최적화 */
  useEffect(() => {
    let isEffectActive = true;
    globalGpsSubscriberCount++;

    const handleVisibilityChange = () => {
      // 백그라운드 전환 시 배터리 절약을 위해 센서 일시 중단
      if (document.visibilityState === 'hidden') {
        stopWatching();
      }
      // 포그라운드 복귀 시 센서 재가동
      else if (status !== 'idle' && isEffectActive) {
        startWatching();
      }
    };

    // 최초 마운트 시 최초 구독자라면 센서 가동 시작
    if (globalGpsSubscriberCount === 1) {
      startWatching();
    }

    document.addEventListener('visibilitychange', handleVisibilityChange);

    return () => {
      isEffectActive = false;
      globalGpsSubscriberCount--;

      // 🚀 [Refinement] 마지막 구독자가 언마운트될 때만 상태 초기화
      if (globalGpsSubscriberCount === 0) {
        stopWatching();

        // 🚨 무한 루프 방지: 이미 idle이 아닐 때만 변경 요청
        if (status !== 'idle') {
          setStatus('idle');
        }
      }

      document.removeEventListener('visibilitychange', handleVisibilityChange);
    };
  }, [startWatching, stopWatching, setStatus, status]);
}

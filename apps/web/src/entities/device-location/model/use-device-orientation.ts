import { useCallback, useEffect } from 'react';
import { useDeviceLocationStore } from '@/entities/device-location/model/device-location.store';

/** * [Singleton 관리를 위한 전역 카운트]
 * 센서 하드웨어 리소스를 공유하는 컴포넌트 수를 추적합니다.
 */
let globalOrientationSubscriberCount = 0;

/**
 * [use-device-orientation]
 * 기기 나침반 센서를 감시하고 전역 스토어와 동기화하는 상위 호환 싱글톤 훅입니다.
 */
export function useDeviceOrientation() {
  const { setCompassHeading, isOrientationStarted, setOrientationStarted } =
    useDeviceLocationStore();

  /** 1. 기존의 정교한 데이터 처리 로직 (누락 없음) */
  const handleOrientation = useCallback(
    (e: DeviceOrientationEvent) => {
      const webkitHeading = (e as any).webkitCompassHeading;
      if (webkitHeading !== undefined) {
        setCompassHeading(webkitHeading);
        return;
      }

      if (e.alpha !== null) {
        const heading = (360 - e.alpha) % 360;
        setCompassHeading(heading);
      }
    },
    [setCompassHeading],
  );

  /** 2. 리스너 제거 함수 (참조 카운트가 0일 때만 실행) */
  const removeOrientationListener = useCallback(() => {
    window.removeEventListener('deviceorientation', handleOrientation, true);
    window.removeEventListener(
      'deviceorientationabsolute',
      handleOrientation,
      true,
    );
  }, [handleOrientation]);

  /** 3. 권한 요청 및 센서 가동 (기존 중복 가드 유지) */
  const requestPermission = useCallback(async () => {
    if (isOrientationStarted) return true;

    if (
      typeof (DeviceOrientationEvent as any).requestPermission === 'function'
    ) {
      try {
        const permission = await (
          DeviceOrientationEvent as any
        ).requestPermission();
        if (permission === 'granted') {
          window.addEventListener('deviceorientation', handleOrientation, true);
          setOrientationStarted(true);
          return true;
        }
      } catch (error) {
        console.error('Orientation Permission Error:', error);
      }
    } else {
      const eventName =
        'ondeviceorientationabsolute' in window
          ? 'deviceorientationabsolute'
          : 'deviceorientation';
      window.addEventListener(eventName, handleOrientation, true);
      setOrientationStarted(true);
      return true;
    }
    return false;
  }, [handleOrientation, isOrientationStarted, setOrientationStarted]);

  /** 4. 라이프사이클 및 자동 시작 로직 */
  useEffect(() => {
    let isEffectActive = true;
    globalOrientationSubscriberCount++;

    const isIOS =
      typeof (DeviceOrientationEvent as any).requestPermission === 'function';

    // 안드로이드/데스크톱 등 자동 시작 가능 환경 대응
    if (!isOrientationStarted && !isIOS && isEffectActive) {
      requestPermission().catch(console.error);
    }

    return () => {
      isEffectActive = false;
      globalOrientationSubscriberCount--;

      // 🧹 마지막 구독자가 떠날 때만 완벽하게 뒷정리
      if (globalOrientationSubscriberCount === 0) {
        removeOrientationListener();

        // 🚨 핵심 보완: 현재 상태가 true일 때만 false로 바꿈 (무한 루프 방지)
        if (isOrientationStarted) {
          setOrientationStarted(false);
          setCompassHeading(null);
        }
      }
    };
  }, [
    isOrientationStarted,
    requestPermission,
    setOrientationStarted,
    setCompassHeading,
    removeOrientationListener,
  ]);

  return { requestPermission, isPermissionGranted: isOrientationStarted };
}

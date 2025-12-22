import { useState, useCallback, useEffect } from 'react';

export function useDeviceOrientation() {
  const [compassHeading, setCompassHeading] = useState<number | null>(null);
  const [isPermissionGranted, setIsPermissionGranted] = useState(false);

  // 1. 이벤트 핸들러 분리
  const handleOrientation = useCallback((e: DeviceOrientationEvent) => {
    // 1. iOS (가장 정확한 절대 방위)
    const webkitHeading = (e as any).webkitCompassHeading;
    if (webkitHeading !== undefined) {
      setCompassHeading(webkitHeading);
      return;
    }

    // 2. Android / Chrome (deviceorientationabsolute 이벤트 대응)
    // absolute가 true이거나 alpha가 존재할 때
    if (e.alpha !== null) {
      // 🚀 보통 Android는 시계 반대방향이므로 360 - alpha를 사용
      // 하지만 센서 데이터를 더 부드럽게 쓰려면 그대로 사용하고
      // UI에서 보정하는 것이 나을 때도 있습니다.
      const heading = (360 - e.alpha) % 360;
      setCompassHeading(heading);
    }
  }, []);

  // 2. 권한 요청 함수 (버튼 클릭 핸들러에서 실행할 것)
  const requestPermission = useCallback(async () => {
    // iOS 권한 요청 처리
    if (
      typeof (DeviceOrientationEvent as any).requestPermission === 'function'
    ) {
      try {
        const permission = await (
          DeviceOrientationEvent as any
        ).requestPermission();
        if (permission === 'granted') {
          window.addEventListener('deviceorientation', handleOrientation, true);
          setIsPermissionGranted(true);
          return true;
        }
      } catch (error) {
        console.error('Orientation Permission Denied:', error);
      }
    } else {
      // Android 및 일반 브라우저
      window.addEventListener(
        'deviceorientationabsolute',
        handleOrientation,
        true,
      );
      // 절대값 지원 안할 경우 일반 이벤트라도 등록
      window.addEventListener('deviceorientation', handleOrientation, true);
      setIsPermissionGranted(true);
      return true;
    }
    return false;
  }, [handleOrientation]);

  // 언마운트 시 이벤트 제거
  useEffect(() => {
    return () => {
      window.removeEventListener('deviceorientation', handleOrientation);
      window.removeEventListener(
        'deviceorientationabsolute',
        handleOrientation,
      );
    };
  }, [handleOrientation]);

  return { compassHeading, requestPermission, isPermissionGranted };
}

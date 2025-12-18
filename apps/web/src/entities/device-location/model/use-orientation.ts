import { useState, useCallback, useEffect } from 'react';

export function useDeviceOrientation() {
  const [compassHeading, setCompassHeading] = useState<number | null>(null);
  const [isPermissionGranted, setIsPermissionGranted] = useState(false);

  // 1. 이벤트 핸들러 분리
  const handleOrientation = useCallback((e: DeviceOrientationEvent) => {
    // iOS 전용 나침반 값
    const webkitHeading = (e as any).webkitCompassHeading;

    if (webkitHeading !== undefined) {
      setCompassHeading(webkitHeading);
    }
    // Android 및 표준: absolute 모드일 때 alpha 사용
    else if (e.alpha !== null) {
      // Android는 시계 반대방향이므로 변환 필요
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

import { useState, useEffect, useMemo } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { useMapStore } from './map-store';
import { useDeviceLocationStore } from '@/entities/device-location/model/device-location.store';
import { projection } from '@pixelet/shared/shared/lib/map/projection';

const { project } = projection;

/**
 * [use-map-navigation]
 * 지도 내비게이션 및 위치 제어에 필요한 상태와 액션들을 성능 최적화(Shallow) 처리하여 제공하는 커스텀 훅입니다.
 */
export function useMapNavigation() {
  const deviceLocation = useDeviceLocationStore((state) => state.location);
  const rawAccuracy = useDeviceLocationStore((state) => state.accuracy);
  const compassHeading = useDeviceLocationStore(
    (state) => state.compassHeading,
  );

  const store = useMapStore(
    useShallow((state) => ({
      center: state.center,
      setCenter: state.setCenter,
      setCenterByLocation: state.setCenterByLocation,

      viewState: state.viewState,
      setViewState: state.setViewState,

      origin: state.origin,
      setOrigin: state.setOrigin,

      triggerDistanceReset: state.triggerDistanceReset,
      triggerOrientationReset: state.triggerOrientationReset,

      hoveredPos: state.hoveredPos,
      setHoveredPos: state.setHoveredPos,

      isFollowMode: state.isFollowMode,
      setIsFollowMode: state.setIsFollowMode,

      isInteracting: state.isInteracting,
      setIsInteracting: state.setIsInteracting,

      currentHour: state.currentHour,
      getThemeKey: state.getThemeKey,
    })),
  );

  /**
   * [get-constrained-accuracy]
   * GPS 수신 불량으로 인한 비정상적인 오차 범위를 보정하여 UI에 적합한 수치로 변환합니다.
   */
  const accuracy = useMemo(() => {
    if (rawAccuracy === null || rawAccuracy === undefined) return 10;
    return Math.min(Math.max(rawAccuracy, 4), 50);
  }, [rawAccuracy]);

  /**
   * [get-absolute-positions]
   * 위경도 기반의 기기 위치를 절대 미터 좌표계로 변환합니다.
   * 렌더링 컴포넌트는 이 값을 받아 useRelativePosition()을 통해 로컬 좌표로 변환해야 합니다.
   */
  const devicePos = useMemo(
    () => (deviceLocation ? project(deviceLocation) : null),
    [deviceLocation],
  );

  // 🗑️ [Deprecated] localDevicePos, localHoveredPos 제거
  // 사유: UserMarker 등 소비처에서 useRelativePosition을 통해 직접 변환하는 것이
  // Floating Origin 정책(Z축 포함, Swizzling 분리)에 부합함.

  const [isOffCenter, setIsOffCenter] = useState(false);

  /**
   * [apply-hysteresis-logic]
   * 기기 위치와 지도 중심점 사이의 이탈/복귀 판정.
   * JS Number(64-bit) 연산이므로 절대 좌표 간 직접 뺄셈도 정밀도 문제가 없습니다.
   */
  useEffect(() => {
    if (!devicePos || !store.center) return;

    // 절대 좌표 차이 계산 (미터 단위)
    const dx = devicePos.x - store.center.x;
    const dy = devicePos.y - store.center.y;
    const dist = Math.sqrt(dx * dx + dy * dy);

    if (!isOffCenter && dist > 10) {
      setIsOffCenter(true);
    } else if (isOffCenter && dist < 3) {
      setIsOffCenter(false);
    }
  }, [
    devicePos?.x,
    devicePos?.y,
    store.center?.x,
    store.center?.y,
    isOffCenter,
  ]);

  const canShowLocationControl = isOffCenter && !store.isFollowMode;

  const themeKey = useMemo(
    () => store.getThemeKey(),
    [store.currentHour, store.getThemeKey],
  );

  return {
    ...store,
    devicePos,
    accuracy,
    isOffCenter,
    deviceLocation,
    compassHeading,
    canShowLocationControl,
    themeKey,
  };
}

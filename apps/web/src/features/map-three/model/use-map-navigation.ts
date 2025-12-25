import { useState, useEffect, useMemo } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { useMapStore } from './map-store';
import { useDeviceLocationStore } from '@/entities/device-location/model/device-location.store';
import { projection } from '@/shared/lib/geo/projection';

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
      setOriginByLocation: state.setOriginByLocation,

      originOffset: state.originOffset,
      setOriginOffset: state.setOriginOffset,

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
   */
  const devicePos = useMemo(
    () => (deviceLocation ? project(deviceLocation) : null),
    [deviceLocation, project],
  );

  /**
   * [get-local-positions]
   * Floating Origin 지침에 따라 절대 좌표를 현재 씬 원점 기준의 로컬 좌표로 변환합니다.
   */
  const localDevicePos = useMemo(() => {
    if (!devicePos || !store.originOffset) return null;
    return {
      x: devicePos.x - store.originOffset.x,
      z: devicePos.z - store.originOffset.z,
    };
  }, [devicePos, store.originOffset]);

  const localHoveredPos = useMemo(() => {
    if (!store.hoveredPos || !store.originOffset) return null;
    return {
      x: store.hoveredPos.x - store.originOffset.x,
      z: store.hoveredPos.z - store.originOffset.z,
    };
  }, [store.hoveredPos, store.originOffset]);

  const [isOffCenter, setIsOffCenter] = useState(false);

  /**
   * [apply-hysteresis-logic]
   * 기기 위치와 지도 중심점 사이의 이탈/복귀 판정 기준선을 다르게 설정하여 UI 깜빡임을 방지합니다.
   * - 이탈(Off-Center): 중심에서 10m 이상 멀어질 때 발생.
   * - 복귀(On-Center): 중심에서 3m 이내로 들어올 때 해제.
   */
  useEffect(() => {
    if (!devicePos || !store.center) return;

    const dist = Math.sqrt(
      Math.pow(devicePos.x - store.center.x, 2) +
        Math.pow(devicePos.z - store.center.z, 2),
    );

    if (!isOffCenter && dist > 10) {
      setIsOffCenter(true);
    } else if (isOffCenter && dist < 3) {
      setIsOffCenter(false);
    }
  }, [
    devicePos?.x,
    devicePos?.z,
    store.center?.x,
    store.center?.z,
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
    localDevicePos,
    localHoveredPos,
    accuracy,
    isOffCenter,
    deviceLocation,
    compassHeading,
    canShowLocationControl,
    themeKey,
  };
}

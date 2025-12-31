import { useState, useEffect, useMemo } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { useMapStore } from './map-store';
import { useDeviceLocationStore } from '@/entities/device-location/model/device-location.store';
import { projection } from '@pixelet/shared/shared/lib/map/projection';

const { project } = projection;

/**
 * [use-map-navigation]
 * 지도 내비게이션, 기기 위치 추적, 카메라 제어에 필요한 상태와 액션들을 통합 관리하는 커스텀 훅입니다.
 *
 * 🏗️ [Architecture Note]
 * - Facade Pattern: MapStore(전역 상태)와 DeviceLocationStore(센서 데이터)를 결합하여 단일 인터페이스로 제공합니다.
 * - Performance: useShallow를 사용하여 스토어의 불필요한 리렌더링을 방지합니다.
 */
export function useMapNavigation() {
  // --- [1. Device Sensor Data] ---
  /** GPS 원본 위경도 데이터 */
  const deviceLocation = useDeviceLocationStore((state) => state.location);
  /** GPS 수신 정확도 (미터 단위, 원본) */
  const rawAccuracy = useDeviceLocationStore((state) => state.accuracy);
  /** 나침반 헤딩 각도 (0~360) */
  const compassHeading = useDeviceLocationStore(
    (state) => state.compassHeading,
  );

  // --- [2. Map State Subscription] ---
  const store = useMapStore(
    useShallow((state) => ({
      // [Coordinates & Navigation]
      /** 카메라가 바라보는 절대 중심 좌표 */
      center: state.center,
      setCenter: state.setCenter,

      // [Camera View]
      /** 줌, 회전, 고도 등 시각적 상태 */
      viewState: state.viewState,
      setViewState: state.setViewState,

      // [Floating Origin System]
      /** 렌더링 정밀도 유지를 위한 동적 원점 */
      origin: state.origin,
      setOrigin: state.setOrigin,

      // [Signals]
      /** 줌 레벨 초기화 트리거 */
      triggerDistanceReset: state.triggerDistanceReset,
      /** 회전 각도(북쪽) 초기화 트리거 */
      triggerOrientationReset: state.triggerOrientationReset,

      // [Interaction]
      /** 마우스 호버 좌표 (Raycasting) */
      hoveredPos: state.hoveredPos,
      setHoveredPos: state.setHoveredPos,

      // [Modes]
      /** 내 위치 따라가기 모드 활성화 여부 */
      isFollowMode: state.isFollowMode,
      setIsFollowMode: state.setIsFollowMode,

      /** 사용자 인터랙션(드래그 등) 중 여부 */
      isInteracting: state.isInteracting,
      setIsInteracting: state.setIsInteracting,

      // [Environment]
      /** 현재 가상 시간 및 테마 */
      currentHour: state.currentHour,
      getThemeKey: state.getThemeKey,
    })),
  );

  /**
   * [get-constrained-accuracy]
   * GPS 수신 불량(튀는 값)이나 너무 작은 값(비현실적 정밀도)을 보정하여
   * UI(파동 효과 등)에 적합한 수치로 변환(Clamping)합니다.
   * - Min: 4m (최소 UI 크기 보장)
   * - Max: 50m (너무 큰 원 방지)
   */
  const accuracy = useMemo(() => {
    if (rawAccuracy === null || rawAccuracy === undefined) return 10;
    return Math.min(Math.max(rawAccuracy, 4), 50);
  }, [rawAccuracy]);

  /**
   * [get-absolute-positions]
   * 위경도(EPSG:4326) 기반의 기기 위치를 평면 투영된 절대 미터 좌표계(Local Cartesian)로 변환합니다.
   * * ⚠️ [Important]
   * 이 값은 '절대 좌표(Absolute)'입니다. 렌더링 시에는 반드시
   * `useRelativePosition(devicePos)`를 통해 `origin`을 뺀 로컬 좌표로 변환해야 합니다.
   */
  const devicePos = useMemo(
    () => (deviceLocation ? project(deviceLocation) : null),
    [deviceLocation],
  );

  // 🗑️ [Deprecated] localDevicePos, localHoveredPos 제거됨
  // 사유: 소비처(UserMarker 등)에서 useRelativePosition 훅을 사용하여
  // 렌더링 시점에 직접 변환하는 것이 Floating Origin 아키텍처(Z축 일관성, Swizzling 분리)에 부합함.

  /** 현재 보고 있는 화면 중심이 내 위치에서 벗어났는지 여부 */
  const [isOffCenter, setIsOffCenter] = useState(false);

  /**
   * [apply-hysteresis-logic]
   * 기기 위치와 지도 중심점 사이의 거리(Distance)를 계산하여 이탈/복귀를 판정합니다.
   * * 💡 [Hysteresis Pattern]
   * - 이탈 판정: 10m 이상 멀어질 때 (너무 민감한 해제 방지)
   * - 복귀 판정: 3m 이내로 들어올 때 (확실한 복귀 확인)
   * 이로써 경계선에서 상태가 깜빡거리는 현상을 방지합니다.
   * * * JS Number(Double Precision)는 약 9,000조 미터까지 안전하므로,
   * Floating Origin 적용 전의 절대 좌표끼리 뺄셈(dx, dy)을 해도 정밀도 문제는 없습니다.
   */
  useEffect(() => {
    if (!devicePos || !store.center) return;

    // 유클리드 거리 제곱 계산 (sqrt는 비교용으로만 사용)
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

  /**
   * [location-control-visibility]
   * "내 위치로 이동" 버튼을 보여줄지 결정합니다.
   * - 조건: 화면이 내 위치에서 벗어났고(isOffCenter) && 현재 팔로우 모드가 아닐 때
   */
  const canShowLocationControl = isOffCenter && !store.isFollowMode;

  /**
   * [theme-memoization]
   * 현재 시간(currentHour)이 바뀔 때만 테마 키를 재계산합니다.
   */
  const themeKey = useMemo(
    () => store.getThemeKey(),
    [store.currentHour, store.getThemeKey],
  );

  return {
    ...store,
    /** 투영된 기기의 절대 미터 좌표 */
    devicePos,
    /** 보정된 GPS 정확도 (반경) */
    accuracy,
    /** 화면 중심 이탈 여부 */
    isOffCenter,
    /** 원본 GPS 데이터 */
    deviceLocation,
    /** 나침반 헤딩 */
    compassHeading,
    /** 위치 복귀 컨트롤 표시 여부 */
    canShowLocationControl,
    /** 현재 시각적 테마 키 */
    themeKey,
  };
}

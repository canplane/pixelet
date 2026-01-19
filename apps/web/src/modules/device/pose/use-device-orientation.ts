// @pxl/web/modules/device/pose/use-device-orientation.ts

import { useCallback, useEffect } from "react";

import type { f64, u32 } from "@xpute/spec/lib/kit/word.ts";

import { useDeviceOrientationStore } from "./store.ts";

/** * [Singleton 관리를 위한 전역 카운트]
 * 센서 하드웨어 리소스를 공유하는 컴포넌트 수를 추적합니다.
 */
let orientation_subscriber_cnt: u32 = 0;

/** 1. 기존의 정교한 데이터 처리 로직 (누락 없음) */
function handle_orientation(e: DeviceOrientationEvent): void {
  const set_orientation = useDeviceOrientationStore.getState().set_orientation;

  // iOS 우선 heading, 없으면 alpha 기반 fallback
  const heading: f64 | null = (e as any).webkitCompassHeading ??
    (e.alpha !== null ? (360 - e.alpha) % 360 : null);

  set_orientation(heading, e.beta, e.gamma);
}

/** 기기 나침반 센서를 감시하고 전역 스토어와 동기화하는 상위 호환 싱글톤 훅 */
export function useDeviceOrientation() {
  const set_is_tracking_orientation = useDeviceOrientationStore((s) => s.set_is_tracking_orientation);
  const is_tracking_orientation: boolean = useDeviceOrientationStore((s) => s.is_tracking_orientation);

  /** 2. 리스너 제거 함수 (참조 카운트가 0일 때만 실행) */
  const remove_orientation_listener = useCallback(() => {
    window.removeEventListener("deviceorientation", handle_orientation, true);
    window.removeEventListener(
      "deviceorientationabsolute",
      handle_orientation,
      true,
    );
  }, []);

  /** 3. 권한 요청 및 센서 가동 (기존 중복 가드 유지) */
  const request_orientation = useCallback(async () => {
    if (useDeviceOrientationStore.getState().is_tracking_orientation) return true;

    if (
      typeof (DeviceOrientationEvent as any).requestPermission === "function"
    ) {
      try {
        const permission = await (
          DeviceOrientationEvent as any
        ).requestPermission();
        if (permission === "granted") {
          window.addEventListener("deviceorientation", handle_orientation, true);
          set_is_tracking_orientation(true);
          return true;
        }
      } catch (err) {
        console.error("device orientation: permission error:", err);
      }
    } else {
      const eventName = "ondeviceorientationabsolute" in window ? "deviceorientationabsolute" : "deviceorientation";
      window.addEventListener(eventName, handle_orientation, true);
      set_is_tracking_orientation(true);
      return true;
    }
    return false;
  }, [set_is_tracking_orientation]);

  /** 4. 라이프사이클 및 자동 시작 로직 */
  useEffect(() => {
    orientation_subscriber_cnt++;

    const is_iOS: boolean = typeof (DeviceOrientationEvent as any).requestPermission === "function";

    // 안드로이드/데스크톱 등 자동 시작 가능 환경 대응
    if (!useDeviceOrientationStore.getState().is_tracking_orientation && !is_iOS) {
      request_orientation().catch(console.error);
    }

    return () => {
      orientation_subscriber_cnt--;

      // 🧹 마지막 구독자가 떠날 때만 완벽하게 뒷정리
      if (orientation_subscriber_cnt === 0) {
        remove_orientation_listener();

        // 🚨 핵심 보완: 현재 상태가 true일 때만 false로 바꿈 (무한 루프 방지)
        if (useDeviceOrientationStore.getState().is_tracking_orientation) {
          set_is_tracking_orientation(false);
        }
      }
    };
  }, [request_orientation, set_is_tracking_orientation, remove_orientation_listener]);

  return {
    request_orientation,
    is_tracking_orientation,
  };
}

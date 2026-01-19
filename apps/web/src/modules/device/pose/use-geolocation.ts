// @pxl/web/modules/device/pose/use-geolocation.ts

import { useCallback, useEffect } from "react";

import type { u32 } from "@xpute/spec/lib/kit/word.ts";

import { DevicePoseStatus, useDeviceLocationStore } from "./store.ts";

/** * 공유 자원 관리를 위한 전역 구독자 카운트
 * 여러 컴포넌트에서 훅 호출 시 센서 중복 가동을 방지합니다.
 */
let geolocation_subscriber_cnt: u32 = 0;
let geolocation_watch_id: number | null = null;

/** 기기 GPS 센서를 감시하여 전역 스토어와 동기화하는 싱글톤 서비스 훅 */
export function useGeolocation() {
  const set_location = useDeviceLocationStore((s) => s.set_location);
  const set_error = useDeviceLocationStore((s) => s.set_error);
  const set_status = useDeviceLocationStore((s) => s.set_status);

  /** 센서 수신 중단 및 리소스 해제 */
  const stop_watching = useCallback(() => {
    if (geolocation_watch_id !== null) {
      navigator.geolocation.clearWatch(geolocation_watch_id);
      geolocation_watch_id = null;

      useDeviceLocationStore.getState().set_is_tracking_location(false);
    }
  }, []);

  /** 센서 수신 시작 (중복 실행 가드 포함) */
  const start_watching = useCallback(() => {
    // 1. 이미 활성화된 센서가 있다면 가동하지 않음
    if (geolocation_watch_id !== null) return;

    if (!("geolocation" in navigator)) {
      set_error("이 브라우저는 위치 정보를 지원하지 않습니다.");
      return;
    }

    set_status(DevicePoseStatus.LOADING);
    useDeviceLocationStore.getState().set_is_tracking_location(true);

    geolocation_watch_id = navigator.geolocation.watchPosition(
      (pos) => {
        const { latitude, longitude, altitude, accuracy, heading } = pos.coords;

        // 💡 [Adapter] 브라우저 파편화 방지 및 데이터 표준화
        // 1. 위경도는 필수이므로 기본값 없이 추출
        // 2. 고도는 기기에 따라 null일 수 있으므로 우리 도메인 표준(undefined/0)으로 변환
        // 3. 정확도와 이동방향도 null 가드 처리

        set_location(
          [longitude, latitude, altitude ?? undefined],
          accuracy,
          heading, // course
        );
      },
      (err) => {
        useDeviceLocationStore.getState().set_is_tracking_location(false);

        let error_message: string = "위치 정보를 가져올 수 없습니다.";
        if (err.code === err.PERMISSION_DENIED) {
          error_message = "위치 권한이 거부되었습니다.";
        } else if (err.code === err.POSITION_UNAVAILABLE) {
          error_message = "위치 신호를 잡을 수 없습니다.";
        } else if (err.code === err.TIMEOUT) {
          error_message = "수신 시간이 초과되었습니다.";
        }
        set_error(error_message);
      },
      { enableHighAccuracy: true, maximumAge: 0, timeout: 10000 },
    );
  }, [set_location, set_error, set_status]);

  /** 라이프사이클 관리 및 탭 가시성 최적화 */
  useEffect(() => {
    let is_effect_active: boolean = true;
    geolocation_subscriber_cnt++;

    function handle_visibility_change() {
      if (document.visibilityState === "hidden") {
        stop_watching(); // 백그라운드 전환 시 배터리 절약을 위해 센서 일시 중단
      } else if (is_effect_active) {
        start_watching(); // 포그라운드 복귀 시 센서 재가동
      }
    }

    // 최초 마운트 시 최초 구독자라면 센서 가동 시작
    if (geolocation_subscriber_cnt === 1) {
      start_watching();
    }

    document.addEventListener("visibilitychange", handle_visibility_change);

    return () => {
      is_effect_active = false;
      geolocation_subscriber_cnt--;

      if (geolocation_subscriber_cnt === 0) {
        stop_watching();

        // 🚨 무한 루프 방지: 이미 idle이 아닐 때만 변경 요청
        if (useDeviceLocationStore.getState().status !== DevicePoseStatus.IDLE) {
          set_status(DevicePoseStatus.IDLE);
        }
      }

      document.removeEventListener("visibilitychange", handle_visibility_change);
    };
  }, [start_watching, stop_watching, set_status]);
}

// @pxl/web/modules/device/pose/use_geolocation.ts

import { useCallback, useEffect } from "react";

import type { u32 } from "@xpute/core/kit/word.ts";

import { DevicePoseStatus, useDeviceLocationStore } from "./store.ts";

/** Global subscriber count for shared-resource management — prevents
 * redundant sensor activation when multiple components call this hook. */
let geolocation_subscriber_cnt: u32 = 0;
let geolocation_watch_id: number | null = null;

/** Singleton service hook: watches the device's GPS sensor and syncs it to the global store. */
export function useGeolocation() {
  const set_location = useDeviceLocationStore((s) => s.set_location);
  const set_error = useDeviceLocationStore((s) => s.set_error);
  const set_status = useDeviceLocationStore((s) => s.set_status);

  /** Stops sensor reception and releases resources. */
  const stop_watching = useCallback(() => {
    if (geolocation_watch_id !== null) {
      navigator.geolocation.clearWatch(geolocation_watch_id);
      geolocation_watch_id = null;

      useDeviceLocationStore.getState().set_is_tracking_location(false);
    }
  }, []);

  /** Starts sensor reception (guarded against duplicate activation). */
  const start_watching = useCallback(() => {
    // already active — nothing to do
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

        // adapter: normalizes across browser inconsistencies.
        // - lat/lng are required, extracted with no fallback
        // - altitude can be null depending on device, mapped to our domain's undefined/0
        // - accuracy/heading are null-guarded too

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

  /** Lifecycle management and tab-visibility optimization. */
  useEffect(() => {
    let is_effect_active: boolean = true;
    geolocation_subscriber_cnt++;

    function handle_visibility_change() {
      if (document.visibilityState === "hidden") {
        stop_watching(); // pause the sensor in the background to save battery
      } else if (is_effect_active) {
        start_watching(); // resume on returning to the foreground
      }
    }

    // start the sensor on mount if this is the first subscriber
    if (geolocation_subscriber_cnt === 1) {
      start_watching();
    }

    document.addEventListener("visibilitychange", handle_visibility_change);

    return () => {
      is_effect_active = false;
      geolocation_subscriber_cnt--;

      if (geolocation_subscriber_cnt === 0) {
        stop_watching();

        // avoid an infinite loop: only request a change if not already idle
        if (useDeviceLocationStore.getState().status !== DevicePoseStatus.IDLE) {
          set_status(DevicePoseStatus.IDLE);
        }
      }

      document.removeEventListener("visibilitychange", handle_visibility_change);
    };
  }, [start_watching, stop_watching, set_status]);
}

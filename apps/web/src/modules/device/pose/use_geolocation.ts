// @pxl/web/modules/device/pose/use_geolocation.ts

import { useCallback, useEffect } from "react";
import i18next from "i18next";

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
      set_error(i18next.t("device.geolocation.unsupported"));
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
        // clear the dead watch id too, not just the store flag — otherwise
        // start_watching()'s "already active" guard blocks every future
        // retry (including the permission-recovery listener below) forever.
        // clearWatch() first (not just discarding the id) — this callback
        // doesn't end the underlying watchPosition subscription, so without
        // it the "dead" watch keeps calling back (a timeout/unavailable
        // error can still be followed by a late success) *and* a later
        // start_watching() registers a second, fully independent watch on
        // top of it. Two watches racing each other is what turned a single
        // transient error into location flapping null/non-null on every
        // callback from either one — see the device_coord-flicker
        // investigation this comment came out of.
        if (geolocation_watch_id !== null) {
          navigator.geolocation.clearWatch(geolocation_watch_id);
          geolocation_watch_id = null;
        }
        useDeviceLocationStore.getState().set_is_tracking_location(false);

        let error_message: string = i18next.t("device.geolocation.generic_error");
        if (err.code === err.PERMISSION_DENIED) {
          // actionable, not just descriptive — unlike POSITION_UNAVAILABLE/
          // TIMEOUT below, this is the one case the user can actually fix
          // themselves (re-grant permission in browser/OS settings).
          error_message = i18next.t("device.geolocation.permission_denied");
        } else if (err.code === err.POSITION_UNAVAILABLE) {
          error_message = i18next.t("device.geolocation.position_unavailable");
        } else if (err.code === err.TIMEOUT) {
          error_message = i18next.t("device.geolocation.timeout");
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

  /**
   * Recovers from a denied/error state the moment the user grants location
   * access from outside the app (browser site-settings UI) — without this,
   * a denial leaves the store stuck in ERROR forever, since nothing else
   * re-triggers start_watching() until the page reloads.
   */
  useEffect(() => {
    if (!("permissions" in navigator)) return;

    let status: PermissionStatus | undefined;
    let is_effect_active = true;

    const handle_change = () => {
      if (status?.state === "granted" && !useDeviceLocationStore.getState().is_tracking_location) {
        start_watching();
      }
    };

    navigator.permissions.query({ name: "geolocation" }).then((result) => {
      if (!is_effect_active) return;
      status = result;
      status.addEventListener("change", handle_change);
    }).catch(() => {
      // Permissions API unsupported for "geolocation" in this browser — no recovery path, same as before this effect existed.
    });

    return () => {
      is_effect_active = false;
      status?.removeEventListener("change", handle_change);
    };
  }, [start_watching]);
}

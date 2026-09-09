// @pxl/web/modules/device/pose/use_device_orientation.ts

import { useCallback, useEffect } from "react";

import type { f64, u32 } from "@xpute/core/kit/word.ts";

import { DevicePoseStatus, useDeviceOrientationStore } from "./store.ts";

/** iOS Safari's own compass heading, absent from lib.dom.d.ts. */
type IOSOrientationEvent = DeviceOrientationEvent & { webkitCompassHeading?: number };

/** iOS 13+'s permission gate, absent from lib.dom.d.ts. */
type IOSOrientationEventCtor = typeof DeviceOrientationEvent & {
  requestPermission?: () => Promise<"granted" | "denied">;
};

/** Global count for singleton management — tracks how many components share this sensor hardware resource. */
let orientation_subscriber_cnt: u32 = 0;

/**
 * There's no Permissions-API equivalent for orientation (iOS Safari doesn't
 * expose one at all — permission there is only ever regrantable via a fresh
 * user gesture, by design), so a permission revoked mid-session can't be
 * pushed to us like geolocation's `change` event. Track the last real event
 * instead and treat a long gap while nominally tracking as the signal
 * having been lost.
 */
const WATCHDOG_TIMEOUT_MS = 8000;
const WATCHDOG_INTERVAL_MS = 3000;
let last_orientation_event_at: number | null = null;

function handle_orientation(e: DeviceOrientationEvent): void {
  last_orientation_event_at = Date.now();

  const set_orientation = useDeviceOrientationStore.getState().set_orientation;

  // prefer iOS's own heading; fall back to alpha-derived otherwise
  const heading: f64 | null = (e as IOSOrientationEvent).webkitCompassHeading ??
    (e.alpha !== null ? (360 - e.alpha) % 360 : null);

  set_orientation(heading, e.beta, e.gamma);
}

/** Singleton hook: watches the device's compass sensor and syncs it to the global store, across browser variants. */
export function useDeviceOrientation() {
  const set_is_tracking_orientation = useDeviceOrientationStore((s) => s.set_is_tracking_orientation);
  const is_tracking_orientation: boolean = useDeviceOrientationStore((s) => s.is_tracking_orientation);

  /** Removes the listener — only actually run once the subscriber count hits 0. */
  const remove_orientation_listener = useCallback(() => {
    window.removeEventListener("deviceorientation", handle_orientation, true);
    window.removeEventListener(
      "deviceorientationabsolute",
      handle_orientation,
      true,
    );
  }, []);

  /** Requests permission and starts the sensor, guarded against duplicate activation. */
  const request_orientation = useCallback(async () => {
    if (useDeviceOrientationStore.getState().is_tracking_orientation) return true;

    const { set_status, set_error } = useDeviceOrientationStore.getState();

    if (typeof DeviceOrientationEvent === "undefined") {
      set_error("device.orientation.unsupported");
      return false;
    }

    set_status(DevicePoseStatus.LOADING);

    if (typeof (DeviceOrientationEvent as IOSOrientationEventCtor).requestPermission === "function") {
      try {
        const permission = await (DeviceOrientationEvent as IOSOrientationEventCtor).requestPermission!();
        if (permission === "granted") {
          window.addEventListener("deviceorientation", handle_orientation, true);
          last_orientation_event_at = Date.now();
          set_is_tracking_orientation(true);
          return true;
        }
        set_error("device.orientation.permission_denied");
      } catch (err) {
        set_error("device.orientation.permission_error");
        console.error("[device_orientation] permission error:", err);
      }
    } else {
      const eventName = "ondeviceorientationabsolute" in window ? "deviceorientationabsolute" : "deviceorientation";
      window.addEventListener(eventName, handle_orientation, true);
      last_orientation_event_at = Date.now();
      set_is_tracking_orientation(true);
      return true;
    }
    return false;
  }, [set_is_tracking_orientation]);

  /** Lifecycle management and auto-start logic. */
  useEffect(() => {
    orientation_subscriber_cnt++;

    const is_iOS: boolean = typeof DeviceOrientationEvent !== "undefined" &&
      typeof (DeviceOrientationEvent as IOSOrientationEventCtor).requestPermission === "function";

    // auto-start on platforms that allow it (Android, desktop, ...)
    if (!useDeviceOrientationStore.getState().is_tracking_orientation && !is_iOS) {
      request_orientation().catch((err) => console.error("[device_orientation] request failed:", err));
    }

    return () => {
      orientation_subscriber_cnt--;

      // only clean up once the last subscriber leaves
      if (orientation_subscriber_cnt === 0) {
        remove_orientation_listener();

        // avoid an infinite loop: only flip to false if currently true
        if (useDeviceOrientationStore.getState().is_tracking_orientation) {
          set_is_tracking_orientation(false);
        }

        // avoid an infinite loop: only reset if not already idle
        if (useDeviceOrientationStore.getState().status !== DevicePoseStatus.IDLE) {
          useDeviceOrientationStore.getState().set_status(DevicePoseStatus.IDLE);
        }
      }
    };
  }, [request_orientation, set_is_tracking_orientation, remove_orientation_listener]);

  /**
   * See WATCHDOG_TIMEOUT_MS's doc comment — polls for a stalled signal while
   * nominally tracking, AND (unlike geolocation, which has
   * navigator.permissions.query("geolocation") to notice access being
   * restored and restart itself — see use_geolocation.ts) doubles as this
   * sensor's only path back once the watchdog below has marked it lost:
   * there's no Permissions-API equivalent for orientation to listen on, so
   * without this a lost signal stayed lost until the user happened to hit
   * TrackingButton's compass button. Retrying here is safe to repeat
   * indefinitely — request_orientation() itself no-ops while already
   * tracking, so this only ever does anything on the tick right after a
   * stall (or before the very first signal ever arrives).
   */
  useEffect(() => {
    // browsers throttle/stop dispatching deviceorientation entirely for
    // backgrounded tabs — last_orientation_event_at goes stale the whole
    // time we're hidden, so judging elapsed time against it right as the
    // tab becomes visible again would misread "was backgrounded" as
    // "signal lost" and fire a false error. Give it a fresh baseline
    // instead of judging a gap that was never really a stall.
    function handle_visibility_change() {
      if (document.visibilityState === "visible" && last_orientation_event_at !== null) {
        last_orientation_event_at = Date.now();
      }
    }
    document.addEventListener("visibilitychange", handle_visibility_change);

    const interval = setInterval(() => {
      if (document.visibilityState !== "visible") return;

      if (!useDeviceOrientationStore.getState().is_tracking_orientation) {
        // iOS's requestPermission() must run inside a direct user gesture —
        // calling it here would just reject, so recovery there stays
        // manual (the compass button). Every other platform auto-starts
        // with no such restriction, so it's safe to just retry.
        const is_iOS = typeof DeviceOrientationEvent !== "undefined" &&
          typeof (DeviceOrientationEvent as IOSOrientationEventCtor).requestPermission === "function";
        if (!is_iOS) request_orientation().catch((err) => console.error("[device_orientation] request failed:", err));
        return;
      }

      if (last_orientation_event_at === null) return;

      if (Date.now() - last_orientation_event_at > WATCHDOG_TIMEOUT_MS) {
        remove_orientation_listener();
        set_is_tracking_orientation(false);
        useDeviceOrientationStore.getState().set_error("device.orientation.signal_lost");
        last_orientation_event_at = null;
      }
    }, WATCHDOG_INTERVAL_MS);

    return () => {
      clearInterval(interval);
      document.removeEventListener("visibilitychange", handle_visibility_change);
    };
  }, [remove_orientation_listener, set_is_tracking_orientation, request_orientation]);

  return {
    request_orientation,
    is_tracking_orientation,
  };
}

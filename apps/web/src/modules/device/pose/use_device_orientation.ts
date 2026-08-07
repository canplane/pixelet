// @pxl/web/modules/device/pose/use_device_orientation.ts

import { useCallback, useEffect } from "react";

import type { f64, u32 } from "@xpute/core/kit/word.ts";

import { useDeviceOrientationStore } from "./store.ts";

/** Global count for singleton management — tracks how many components share this sensor hardware resource. */
let orientation_subscriber_cnt: u32 = 0;

function handle_orientation(e: DeviceOrientationEvent): void {
  const set_orientation = useDeviceOrientationStore.getState().set_orientation;

  // prefer iOS's own heading; fall back to alpha-derived otherwise
  const heading: f64 | null = (e as any).webkitCompassHeading ??
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

  /** Lifecycle management and auto-start logic. */
  useEffect(() => {
    orientation_subscriber_cnt++;

    const is_iOS: boolean = typeof (DeviceOrientationEvent as any).requestPermission === "function";

    // auto-start on platforms that allow it (Android, desktop, ...)
    if (!useDeviceOrientationStore.getState().is_tracking_orientation && !is_iOS) {
      request_orientation().catch(console.error);
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
      }
    };
  }, [request_orientation, set_is_tracking_orientation, remove_orientation_listener]);

  return {
    request_orientation,
    is_tracking_orientation,
  };
}

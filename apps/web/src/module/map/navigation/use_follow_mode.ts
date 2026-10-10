// @pxl/web/module/map/navigation/use_follow_mode.ts

import { useEffect, useRef } from "react";

import type { f64 } from "@xpute/core/kit/word.ts";
import { wrap } from "@xpute/core/kit/math.ts";

import { DevicePoseStatus, useDeviceLocationStore, useDeviceOrientationStore } from "@/module/device/pose/store.ts";
import { useCameraStore } from "@/module/camera/store.ts";
import { useSettingsStore } from "@/app/setting/store.ts";
import { ground_or_raw } from "@/module/world/terrain/elevation.ts";

import { useMapStore } from "../store.ts";
import { useMapNavigation } from "./use_map_navigation.ts";

/**
 * EMA factor applied per heading sample (not per frame — device orientation
 * events don't arrive frame-locked). Compass heading is inherently noisy
 * (magnetometer jitter, worse indoors/near metal), and the dead-zone filter
 * in device/pose/store.ts only screens out sub-1.5° noise — this smooths
 * what gets through before it ever reaches azimuth, on top of (not instead
 * of) use_view_frame.ts's per-frame ROT_DAMP easing of the view.
 */
const HEADING_SMOOTH_ALPHA: f64 = 0.25;

/** How long either sensor may be away from READY before follow mode gives
 * up. Longer than use_geolocation.ts's own retry delay, so a recoverable gap
 * gets its retry before this fires. */
const SENSOR_GRACE_MS = 12000;

/**
 * While follow mode is active, syncs the user's live position and heading
 * onto the map.
 */
export function useFollowMode() {
  const set_map_center = useMapStore.getState().set_map_center;

  const set_view_state = useCameraStore.getState().set_view_state;
  const set_is_follow_mode = useCameraStore.getState().set_is_follow_mode;
  const is_follow_mode = useCameraStore((state) => state.is_follow_mode);

  const heading = useDeviceOrientationStore((state) => state.orientation?.heading ?? null);

  const device_coord = useMapNavigation().device_coord;

  const smoothed_azimuth = useRef<f64 | null>(null);

  /**
   * Follow mode needs both sensors to mean anything: it's position (GPS)
   * fused with heading (compass). TrackingButton only lets the user
   * *enter* follow mode once both are already READY (see its own
   * is_enabled/request_orientation gating), but nothing previously
   * re-checked that after entry — if either sensor dropped out mid-session
   * (permission revoked, signal lost), the effects below would just stop
   * updating position/azimuth (their own `=== null` guards) and silently
   * freeze the camera facing a heading/position that's no longer actually
   * live, while is_follow_mode stayed true and TrackingButton's button
   * (disabled without READY location) couldn't even be used to back out —
   * only a manual pan/pinch (camera/input.tsx) would. Exiting the moment
   * either sensor stops being READY closes that dead end: same rule the
   * button already enforces on the way in, just enforced continuously
   * instead of only at the click that started it.
   */
  const location_status = useDeviceLocationStore((state) => state.status);
  const orientation_status = useDeviceOrientationStore((state) => state.status);

  useEffect(() => {
    if (!is_follow_mode) return;
    if (location_status === DevicePoseStatus.READY && orientation_status === DevicePoseStatus.READY) return;

    // Not immediately. Either sensor can leave READY for a moment — a GPS fix
    // that misses its window indoors, an orientation gap — and dropping follow
    // mode on the first blip read as the mode switching itself off at random,
    // with nothing on screen to explain it. use_geolocation.ts retries after
    // GEOLOCATION_RETRY_MS, so most of these recover on their own; waiting
    // longer than that lets them.
    //
    // The exit still has to happen eventually. Without it, follow mode stays
    // on with dead sensors: the camera freezes at whatever heading and
    // position it last saw, TrackingButton is disabled (it needs READY) so it
    // cannot be used to back out, and only a manual pan breaks the deadlock.
    const timer = globalThis.setTimeout(() => set_is_follow_mode(false), SENSOR_GRACE_MS);
    return () => globalThis.clearTimeout(timer);
  }, [is_follow_mode, location_status, orientation_status, set_is_follow_mode]);

  // 1. [Position sync] runs only when the GPS coordinate changes
  //
  // No EMA here: it was a second position-smoothing stage stacked on top of
  // companion.tsx's own catch-up lerp, and while walking the two lags
  // compounded into a roughly constant gap between the character and the
  // (raw-GPS) location puck. companion.tsx's follow-mode branch now glides at
  // the world-scroll damp rate — the single smoothing stage — so this just
  // hands map_center the raw fix. Ordinary fix-to-fix jitter is absorbed by
  // that damp; genuinely bad fixes are already dropped by device/pose/
  // store.ts's accuracy guard.
  //
  // ground_or_raw for the same reason do_pan and the GPS recenter button use
  // it (see its doc comment): device_coord's Z is raw GPS altitude, not baked
  // terrain height — regrounding here keeps follow mode agreeing with
  // pan/recenter on where the ground is.
  useEffect(() => {
    if (!is_follow_mode || device_coord === null) return;
    set_map_center(useSettingsStore.getState().ground_snap ? ground_or_raw(device_coord) : device_coord);
  }, [is_follow_mode, device_coord]);

  // dropping the smoothing anchor on exit means re-entering follow mode snaps
  // straight to the current heading instead of easing all the way from
  // wherever it was left off, possibly a while ago.
  useEffect(() => {
    if (!is_follow_mode) {
      smoothed_azimuth.current = null;
    }
  }, [is_follow_mode]);

  // 2. [Heading sync] runs only when the compass angle changes
  useEffect(() => {
    if (!is_follow_mode || heading === null) return;

    const raw_azimuth: f64 = heading * (Math.PI / 180);

    if (smoothed_azimuth.current === null) {
      smoothed_azimuth.current = raw_azimuth;
    } else {
      // shortest signed distance across the wrap (e.g. 359°→1° is a small
      // +2° step, not -358°), eased by HEADING_SMOOTH_ALPHA per sample.
      const delta: f64 = wrap(raw_azimuth - smoothed_azimuth.current, -Math.PI, Math.PI);
      smoothed_azimuth.current += delta * HEADING_SMOOTH_ALPHA;
    }

    set_view_state({ azimuth: smoothed_azimuth.current });
  }, [is_follow_mode, heading]);
}

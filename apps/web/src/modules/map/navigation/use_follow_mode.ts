// @pxl/web/modules/map/navigation/use_follow_mode.ts

import { useEffect, useRef } from "react";

import type { f64 } from "@xpute/core/kit/word.ts";
import { wrap } from "@xpute/core/kit/math.ts";
import { type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { DevicePoseStatus, useDeviceLocationStore, useDeviceOrientationStore } from "@/modules/device/pose/store.ts";
import { useCameraStore } from "@/modules/camera/store.ts";
import { ground_or_raw } from "@/modules/world/terrain/use_elevation.ts";

import { useMapStore } from "../store.ts";
import { useMapNavigation } from "./use_map_navigation.ts";

/**
 * EMA factor applied per heading sample (not per frame — device orientation
 * events don't arrive frame-locked). Compass heading is inherently noisy
 * (magnetometer jitter, worse indoors/near metal), and the dead-zone filter
 * in device/pose/store.ts only screens out sub-1.5° noise — this smooths
 * what gets through before it ever reaches azimuth, on top of (not instead
 * of) use_frame_rotation.ts's per-frame ROT_DAMP easing at the render layer.
 */
const HEADING_SMOOTH_ALPHA: f64 = 0.25;

/**
 * EMA factor applied per GPS fix (same reasoning as HEADING_SMOOTH_ALPHA).
 * device/pose/store.ts's accuracy guard lets through anything under ~100m
 * (or worse, if "improving") — ordinary noisy fixes, not just outliers — so
 * without this, every accuracy wobble drags map_center (and the camera,
 * via use_frame_translation.ts's POS_DAMP) toward it and back.
 */
const POSITION_SMOOTH_ALPHA: f64 = 0.25;

const $pos_delta: PVF3 = { i: 0n, f: 0n };

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
  const smoothed_coord = useRef<PVF3 | null>(null);

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
    if (location_status !== DevicePoseStatus.READY || orientation_status !== DevicePoseStatus.READY) {
      set_is_follow_mode(false);
    }
  }, [is_follow_mode, location_status, orientation_status, set_is_follow_mode]);

  // 1. [Position sync] runs only when the GPS coordinate changes
  useEffect(() => {
    if (!is_follow_mode || device_coord === null) return;

    if (smoothed_coord.current === null) {
      smoothed_coord.current = device_coord;
    } else {
      // exact BigInt-domain delta first (avoids float cancellation over a
      // large absolute coordinate), *then* drop to float meters to scale by
      // the EMA factor — same idiom as pvf3-consuming code elsewhere (e.g.
      // do_pan in camera/input.tsx).
      pvf3.sub($pos_delta, device_coord, smoothed_coord.current);
      const dx: f64 = pvf3.to_real(pvf3.X($pos_delta)) * POSITION_SMOOTH_ALPHA;
      const dy: f64 = pvf3.to_real(pvf3.Y($pos_delta)) * POSITION_SMOOTH_ALPHA;
      const dz: f64 = pvf3.to_real(pvf3.Z($pos_delta)) * POSITION_SMOOTH_ALPHA;

      const prev = smoothed_coord.current;
      smoothed_coord.current = pvf3.gather(
        { i: 0n, f: 0n },
        pvf3.X(prev) + pvf3.from_real(dx),
        pvf3.Y(prev) + pvf3.from_real(dy),
        pvf3.Z(prev) + pvf3.from_real(dz),
      );
    }

    // same fragmentation bug do_pan and the GPS recenter button used to have
    // (see ground_or_raw's doc comment): smoothed_coord tracks the device's
    // raw GPS-sensor altitude, not baked terrain height. Left as-is, this ran
    // on every GPS fix while follow mode was on and keeps re-overwriting
    // map_center's Z with that raw value — including stomping right back
    // over any correction reground_map_center just applied, since this
    // effect fires again on the next fix a moment later. Regrounding here
    // too, on the final coordinate right before it becomes map_center, is
    // what makes follow mode agree with pan/recenter on "where the ground
    // is" instead of only two of the three call sites agreeing.
    set_map_center(useCameraStore.getState().follow_terrain ? ground_or_raw(smoothed_coord.current) : smoothed_coord.current);
  }, [is_follow_mode, device_coord]);

  // dropping the smoothing anchors on exit means re-entering follow mode
  // snaps straight to the current reading instead of easing all the way
  // from wherever it was left off, possibly a while ago.
  useEffect(() => {
    if (!is_follow_mode) {
      smoothed_azimuth.current = null;
      smoothed_coord.current = null;
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

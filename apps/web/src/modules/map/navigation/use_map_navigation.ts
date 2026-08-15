// @pxl/web/modules/map/navigation/use_map_navigation.ts

import { useEffect, useMemo, useState } from "react";
import { useShallow } from "zustand/react/shallow";

import type { f64 } from "@xpute/core/kit/word.ts";

import { type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import * as epsg4326 from "@pxl/shared/spec/space/adapter/4326.ts";

import { type DeviceLocation, useDeviceLocationStore } from "@/modules/device/pose/store.ts";
import { useMapStore } from "../store.ts";

const $navigation_delta: PVF3 = { i: 0n, f: 0n };

export function useMapNavigation() {
  /** Device sensor data. */
  const device_location: DeviceLocation | null = useDeviceLocationStore((state) => state.location);

  /** Map state subscription. */
  const store = useMapStore(
    useShallow((state) => ({
      map_center: state.map_center,
    })),
  );

  /** Device location projected into canonical PVF3 world space. */
  const device_coord: PVF3 | null = useMemo(() => {
    if (!device_location) return null;
    return epsg4326.project({ i: 0n, f: 0n }, device_location.coords);
  }, [device_location]);

  /**
   * Off-center hysteresis over horizontal (X/Y only) distance.
   *
   * Was full 3D distance — found while investigating a report that the GPS
   * button's second tap (recenter -> toggle follow mode) never reached the
   * follow-mode branch: device_coord's Z is raw GPS altitude, map_center's Z
   * is baked terrain elevation from a different source (see ground_or_raw),
   * so real GPS altitude being tens of meters off true elevation could keep
   * that difference alone above the recenter threshold forever, even
   * sitting exactly on the recentered X/Y spot. Navigation here is
   * inherently horizontal (a top-down map), so only X/Y should decide "did
   * the user wander away".
   */
  const [is_off_center, set_is_off_center] = useState(false);
  useEffect(() => {
    if (store.map_center === null || device_coord === null) {
      set_is_off_center(false);
      return;
    }

    // X occupies one complete signed lane/world period, so PVF3 subtraction
    // gives the short delta across the antimeridian instead of a nearly
    // world-wide raw difference. Y remains ordinary/clamped Mercator space.
    pvf3.sub($navigation_delta, device_coord, store.map_center);
    const dx: f64 = pvf3.to_real(pvf3.X($navigation_delta));
    const dy: f64 = pvf3.to_real(pvf3.Y(device_coord) - pvf3.Y(store.map_center));
    const dist: f64 = Math.sqrt(dx * dx + dy * dy);

    if (!is_off_center && dist > 10) {
      set_is_off_center(true);
    } else if (is_off_center && dist < 3) {
      set_is_off_center(false);
    }
  }, [store.map_center, device_coord, is_off_center]);

  return {
    /** Device coordinate in canonical world space. */
    device_coord,

    /** Whether the current center is off from device location. */
    is_off_center,
  };
}

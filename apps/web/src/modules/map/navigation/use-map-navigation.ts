// @pxl/web/modules/map/navigation/use-map-navigation.ts

import { useEffect, useMemo, useState } from "react";
import { useShallow } from "zustand/react/shallow";

import type { f64 } from "@xpute/spec/lib/kit/word.ts";

import { type IVec, type PVF3, pvf3 } from "@pxl/spec/space/pvf3/mod.ts";
import { L4_BASE } from "@pxl/spec/space/indexer.ts";
import * as epsg4326 from "@pxl/spec/space/adapter/4326.ts";

import { type DeviceLocation, useDeviceLocationStore } from "@/modules/device/pose/store.ts";
import { useMapStore } from "../store.ts";

export function useMapNavigation() {
  /** Device sensor data. */
  const device_location: DeviceLocation | null = useDeviceLocationStore((state) => state.location);

  /** Map state subscription. */
  const store = useMapStore(
    useShallow((state) => ({
      map_center: state.map_center,
    })),
  );

  /** L4-aligned render anchor derived from canonical center. */
  const map_origin: IVec = useMemo(() => {
    if (!store.map_center) return 0n;
    return L4_BASE(store.map_center.i);
  }, [store.map_center]);

  /** Device location projected into canonical PVF3 world space. */
  const device_coord: PVF3 | null = useMemo(() => {
    if (!device_location) return null;
    return epsg4326.project({ i: 0n, f: 0n }, device_location.coords);
  }, [device_location]);

  /**
   * Off-center hysteresis over canonical 3D distance.
   * Altitude participates in the distance test.
   */
  const [is_off_center, set_is_off_center] = useState(false);
  useEffect(() => {
    if (store.map_center === null || device_coord === null) {
      set_is_off_center(false);
      return;
    }

    const dist: f64 = Math.sqrt(pvf3.to_real(pvf3.dist_sq(device_coord, store.map_center)));

    if (!is_off_center && dist > 10) {
      set_is_off_center(true);
    } else if (is_off_center && dist < 3) {
      set_is_off_center(false);
    }
  }, [store.map_center, device_coord, is_off_center]);

  return {
    map_origin,
    /** Device coordinate in canonical world space. */
    device_coord,

    /** Whether the current center is off from device location. */
    is_off_center,
  };
}

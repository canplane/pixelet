// @pxl/web/modules/map/navigation/use-map-bootstrap.ts

import { useEffect } from "react";

import { type PVF3, pvf3 } from "@pixelet/shared/spec/space/pvf3/mod.ts";

import { with_ele } from "@/modules/world/terrain/use-elevation.ts";
import { type SubscribeWake, wait_until } from "@/shared/lib/store/wait-until.ts";
import { useMapStore } from "../store.ts";
import { useMapNavigation } from "./use-map-navigation.ts";
import { useTileStore } from "@/modules/world/terrain/tile-store.ts";

const subscribe_ele_wake: SubscribeWake = (wake) => useTileStore.subscribe(wake);

export function useMapBootstrap() {
  const set_map_center = useMapStore.getState().set_map_center;
  const { device_coord } = useMapNavigation();

  useEffect(() => {
    if (device_coord === null) return;

    const seed: PVF3 = pvf3.gather({ i: 0n, f: 0n }, pvf3.X(device_coord), pvf3.Y(device_coord), 0n);

    const cur = useMapStore.getState().map_center;
    if (cur === null) {
      set_map_center(seed);
    } else if (pvf3.X(cur) !== pvf3.X(seed) || pvf3.Y(cur) !== pvf3.Y(seed)) {
      return;
    }

    return wait_until<PVF3>(
      () => {
        return with_ele({ i: seed.i, f: seed.f }, seed.i, seed.f);
      },
      subscribe_ele_wake,
      (grounded) => {
        const cur = useMapStore.getState().map_center;
        if (cur === null) {
          useMapStore.getState().set_map_center(grounded);
          return;
        }

        if (pvf3.X(cur) !== pvf3.X(seed) || pvf3.Y(cur) !== pvf3.Y(seed)) {
          return;
        }
        useMapStore.getState().set_map_center(grounded);
      },
    );
  }, [device_coord]);
}

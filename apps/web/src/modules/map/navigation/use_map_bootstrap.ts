// @pxl/web/modules/map/navigation/use_map_bootstrap.ts

import { useEffect } from "react";

import { type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import * as epsg4326 from "@pxl/shared/spec/space/adapter/4326.ts";
import type { G4326 } from "@pxl/shared/spec/space/adapter/4326.ts";

import { ground_pivot } from "@/modules/world/terrain/elevation.ts";
import { type SubscribeWake, wait_until } from "@/shared/lib/store/wait_until.ts";
import { useMapStore } from "../store.ts";
import { useMapNavigation } from "./use_map_navigation.ts";
import { useTileStore } from "@/modules/world/terrain/store.ts";
import { DevicePoseStatus, useDeviceLocationStore } from "@/modules/device/pose/store.ts";

const subscribe_ele_wake: SubscribeWake = (wake) => useTileStore.subscribe(wake);

// Seoul City Hall — the approximate coordinate used when GPS genuinely
// never comes up (permission denied, no signal, timeout, unsupported
// browser), instead of leaving map_center null forever and never drawing
// any terrain at all. Never touches the device/pose store — no separate
// "pretend we have a real fix" flag (like is_fallback); this only seeds
// map_center itself with this coordinate. device_coord stays null, so
// LocationPuck naturally has nothing to show (it never gets a real fix to
// remember — see its own has_ground gate), and the GPS recenter button
// naturally stays disabled too (tracking_button.tsx's is_enabled) —
// both fall out for free from the existing state combination.
const FALLBACK_G4326: G4326 = [126.9780, 37.5665];
const FALLBACK_SEED: PVF3 = epsg4326.project({ i: 0n, f: 0n }, FALLBACK_G4326);

function is_at(a: PVF3, b: PVF3): boolean {
  return pvf3.X(a) === pvf3.X(b) && pvf3.Y(a) === pvf3.Y(b);
}

export function useMapBootstrap() {
  const set_map_center = useMapStore.getState().set_map_center;
  const { device_coord } = useMapNavigation();
  const location_status = useDeviceLocationStore((state) => state.status);

  useEffect(() => {
    // prefer the real device position. Only reach for the Seoul fallback
    // once GPS has genuinely given up (ERROR — not merely still
    // LOADING/IDLE) *and* we've never established any center at all. A
    // later GPS dropout, after a real fix already seeded map_center,
    // intentionally does nothing here and just leaves the camera wherever
    // it last was, instead of teleporting it to Seoul — this hook doesn't
    // need to remember *why* the current map_center exists, it just
    // re-derives the best available seed every run and lets a real
    // device_coord always win once one shows up.
    let seed_source: PVF3 | null = null;
    if (device_coord !== null) {
      seed_source = pvf3.gather({ i: 0n, f: 0n }, pvf3.X(device_coord), pvf3.Y(device_coord), 0n);
    } else if (useMapStore.getState().map_center === null && location_status === DevicePoseStatus.ERROR) {
      seed_source = FALLBACK_SEED;
    }
    if (seed_source === null) return;
    const seed = seed_source;

    const cur = useMapStore.getState().map_center;
    // sitting exactly at the fallback seed counts as "nothing real yet"
    // too — otherwise a real fix landing after the fallback kicked in
    // would fall into the "user must have panned away" branch below and
    // never actually take over.
    if (cur === null || is_at(cur, FALLBACK_SEED)) {
      set_map_center(seed);
    } else if (!is_at(cur, seed)) {
      return;
    }

    return wait_until<PVF3>(
      () => {
        return ground_pivot({ i: seed.i, f: seed.f }, seed.i, seed.f);
      },
      subscribe_ele_wake,
      (grounded) => {
        const cur = useMapStore.getState().map_center;
        if (cur === null || is_at(cur, FALLBACK_SEED)) {
          useMapStore.getState().set_map_center(grounded);
          return;
        }

        if (!is_at(cur, seed)) return;
        useMapStore.getState().set_map_center(grounded);
      },
    );
  }, [device_coord, location_status]);
}

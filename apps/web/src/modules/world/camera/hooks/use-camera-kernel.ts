// @pxl/web/modules/world/camera/hooks/use-camera-kernel.ts

import { useRef } from "react";
import { OrbitControls as OrbitControlsImpl } from "three-stdlib";

import type { f64 } from "@xpute/spec/lib/kit/word.ts";

import { ivec, projection, type PVF3 } from "@pxl/spec/space/pvf3/mod.ts";
import type { V3 } from "@pxl/spec/space/v3.ts";

import { useMapStore } from "@/modules/map/navigation/map-store.ts";
import { useMapNavigation } from "@/modules/map/navigation/use-map-navigation.ts";
import { useCameraInit } from "./use-camera-init.ts";
import { useCameraFlight } from "./use-camera-flight.ts";
import { useCameraFrame } from "./use-camera-frame.ts";
import { useCameraGroundAlign } from "./use-camera-ground-align.ts";

/**
 * Aggregates camera bootstrap, flight planning, terrain alignment, and frame sync.
 */
export function useCameraKernel() {
  const controls_ref = useRef<OrbitControlsImpl>(null);

  const set_map_center = useMapStore.getState().set_map_center;
  const set_view_state = useMapStore.getState().set_view_state;
  const set_is_flying = useMapStore.getState().set_is_flying;
  const set_is_follow_mode = useMapStore.getState().set_is_follow_mode;

  const map_origin = useMapNavigation().map_origin;

  useCameraInit();
  const { fly_target } = useCameraFlight({ controls_ref });
  useCameraGroundAlign({ controls_ref, fly_target });
  useCameraFrame({ controls_ref, fly_target });

  /** Transfers camera ownership to direct user interaction. */
  const on_interaction_start = () => {
    set_is_follow_mode(false);
    set_is_flying(false);
  };

  /** Commits the current local camera state back into canonical center/view intent. */
  const on_interaction_end = () => {
    const controls: OrbitControlsImpl | null = controls_ref.current;
    if (!controls) return;

    const { target } = controls;
    const target_v: V3 = [target.x, target.y, target.z];

    if (!target_v.every(Number.isFinite)) return;

    const center: PVF3 = projection.project({ i: 0n, f: 0n }, ...target_v);
    center.i = ivec.add(center.i, map_origin);

    const dist: f64 = controls.object.position.distanceTo(controls.target);
    const height: f64 = controls.object.position.z - controls.target.z;
    const polar_angle: f64 = controls.getPolarAngle();
    let bearing: f64 = -controls.getAzimuthalAngle() * (180 / Math.PI);
    if (bearing <= -180) bearing += 360;
    if (bearing > 180) bearing -= 360;

    set_view_state({ dist, height, bearing, polar_angle });
    set_map_center(center);
  };

  return { controls_ref, on_interaction_start, on_interaction_end };
}

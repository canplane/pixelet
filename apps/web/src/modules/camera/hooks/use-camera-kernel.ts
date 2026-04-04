// @pxl/web/modules/camera/hooks/use-camera-kernel.ts

import { useRef } from "react";
import { OrbitControls as OrbitControlsImpl } from "three-stdlib";

import { ivec, projection, type PVF3 } from "@pxl/spec/space/pvf3/mod.ts";
import { fill3, set3, type V3 } from "@pxl/spec/space/v3.ts";

import { useMapStore } from "@/modules/map/store.ts";
import { useMapNavigation } from "@/modules/map/navigation/use-map-navigation.ts";
import { CameraStatus, useCameraStore } from "../store.ts";
import { useCameraInit } from "./use-camera-init.ts";
import { type CameraChangeSample, type CameraViewSample, is_camera_changed, USER_CAMERA_CHANGE_THRESHOLD } from "../lib/camera-change.ts";
import { useCameraFlight } from "./use-camera-flight.ts";
import { useCameraFrame } from "./use-camera-frame.ts";
import { useCameraGroundAlign } from "./use-camera-ground-align.ts";

/**
 * Aggregates camera bootstrap, flight planning, terrain alignment, and frame sync.
 */
export function useCameraKernel() {
  const target_v_reg = useRef<V3>(fill3(0));

  const controls_ref = useRef<OrbitControlsImpl>(null);
  const did_drag_ref = useRef(false);
  const is_flying_ref = useRef(false);

  const set_map_center = useMapStore.getState().set_map_center;

  const set_view_state = useCameraStore.getState().set_view_state;
  const set_is_follow_mode = useCameraStore.getState().set_is_follow_mode;
  const set_camera_status = useCameraStore.getState().set_camera_status;
  const clear_hovered_coord = useCameraStore.getState().clear_hovered_coord;

  const map_origin = useMapNavigation().map_origin;

  useCameraInit();
  const interaction_0 = useRef<CameraChangeSample | null>(null);
  const set_is_flying = (is_flying: boolean) => {
    is_flying_ref.current = is_flying;
  };

  const { fly_target } = useCameraFlight({ controls_ref, set_is_flying, map_origin });
  const { sample_camera_change, mark_interaction_start, handle_change } = useCameraFrame({
    controls_ref,
    fly_target,
    is_flying_ref,
    set_is_flying,
  });
  useCameraGroundAlign({ controls_ref, fly_target, is_flying_ref, map_origin });

  /** Transfers camera ownership to direct user interaction. */
  function on_interaction_start() {
    did_drag_ref.current = false;
    set_camera_status(CameraStatus.DRAG);

    interaction_0.current = mark_interaction_start();
    set_is_follow_mode(false);
    set_is_flying(false);
  }

  function on_interaction_change() {
    const controls: OrbitControlsImpl | null = controls_ref.current;
    if (!controls) return;

    const cur = handle_change();

    if (!interaction_0.current) return;

    if (!cur) return;

    if (is_camera_changed(cur, interaction_0.current, USER_CAMERA_CHANGE_THRESHOLD)) {
      did_drag_ref.current = true;
      clear_hovered_coord();
    }
  }

  /** Commits the current local camera state back into canonical center/view intent. */
  function on_interaction_end() {
    const controls: OrbitControlsImpl | null = controls_ref.current;
    if (!controls) return;

    const { target } = controls;
    const target_v: V3 = set3(target.x, target.y, target.z, target_v_reg.current);

    if (!target_v.every(Number.isFinite)) return;

    const cur: CameraViewSample = sample_camera_change(controls);

    set_view_state(cur);

    const hovered_coord = useCameraStore.getState().hovered_coord;
    if (!did_drag_ref.current && hovered_coord) {
      set_map_center(hovered_coord);
    } else {
      const center: PVF3 = projection.project({ i: 0n, f: 0n }, target_v[0], target_v[1], target_v[2]);
      center.i = ivec.add(center.i, map_origin);
      set_map_center(center);
    }

    interaction_0.current = null;
    did_drag_ref.current = false;
    set_camera_status(CameraStatus.IDLE);
    clear_hovered_coord();
  }

  return { controls_ref, on_interaction_start, on_interaction_change, on_interaction_end };
}

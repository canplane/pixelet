// @pxl/web/modules/world/camera/hooks/use-camera-flight.ts

import { type RefObject, useEffect, useRef } from "react";
import * as THREE from "three";
import { useThree } from "@react-three/fiber";
import { OrbitControls as OrbitControlsImpl } from "three-stdlib";

import type { f64 } from "@xpute/spec/lib/kit/word.ts";

import { ivec, pvf3 } from "@pxl/spec/space/pvf3/mod.ts";

import { useMapStore } from "@/modules/map/navigation/map-store.ts";
import { useMapNavigation } from "@/modules/map/navigation/use-map-navigation.ts";
import { camera_cfg, CameraControlMode } from "../../render/config.ts";
import { type CameraFlyTarget, derive_camera_pose } from "../../render/camera.ts";

const { DEFAULT_DIST, MIN_DIST, DEFAULT_POLAR_ANGLE, TRAVEL_DIST_THRES } = camera_cfg;

type UseCameraFlightProps = {
  controls_ref: RefObject<OrbitControlsImpl | null>;
};
export function useCameraFlight(props: UseCameraFlightProps) {
  const { controls_ref } = props;

  const map_center = useMapStore((state) => state.map_center);
  const view_state = useMapStore((state) => state.view_state);
  const set_view_state = useMapStore.getState().set_view_state;
  const camera_control_mode = useMapStore((state) => state.camera_control_mode);
  const set_is_flying = useMapStore.getState().set_is_flying;
  const reset_signal = useMapStore((state) => state.camera_reset_signal);

  const map_origin = useMapNavigation().map_origin;

  const { camera } = useThree();

  /** Last camera intent key that has already been planned/applied. */
  const arrived_key = useRef<string>("");

  /** Mutable interpolation destination consumed by the camera frame loop. */
  const fly_target = useRef<CameraFlyTarget>({
    dst_pos: new THREE.Vector3(),
    dst_look_at: new THREE.Vector3(),
  });

  /** Plans either an immediate snap or an interpolated move to the destination pose. */
  const plan_flight = (dst_pos: THREE.Vector3, dst_look_at: THREE.Vector3, opts: { snap?: boolean } = {}) => {
    const controls: OrbitControlsImpl | null = controls_ref.current;
    if (!controls) return;

    const { snap = false } = opts;

    if (snap) {
      camera.position.copy(dst_pos);
      controls.target.copy(dst_look_at);
      controls.update();
      set_is_flying(false);
    } else {
      fly_target.current.dst_pos.copy(dst_pos);
      fly_target.current.dst_look_at.copy(dst_look_at);
      set_is_flying(true);
    }
  };

  /** Re-plans camera motion whenever the canonical center changes. */
  useEffect((): void => {
    const controls: OrbitControlsImpl | null = controls_ref.current;
    if (!map_center || !controls) {
      if (camera_control_mode === CameraControlMode.USER) set_is_flying(false);
      return;
    }

    if (camera_control_mode === CameraControlMode.USER) return;

    const bound_key = [pvf3.repr(map_center), ivec.repr(map_origin)].join("|");
    if (arrived_key.current === bound_key) return;

    const dist: f64 = view_state.dist > MIN_DIST ? view_state.dist : DEFAULT_DIST;
    const polar_angle: f64 = view_state.polar_angle || DEFAULT_POLAR_ANGLE;

    const { look_at, pos } = derive_camera_pose(map_center, map_origin, dist, view_state.bearing, polar_angle);

    const dst_look_at = new THREE.Vector3(...look_at);
    const dst_pos = new THREE.Vector3(...pos);
    const travel_dist: f64 = controls.target.distanceTo(dst_look_at);

    const is_first_plan = arrived_key.current === "";
    plan_flight(dst_pos, dst_look_at, { snap: is_first_plan || travel_dist > TRAVEL_DIST_THRES });

    arrived_key.current = bound_key;
  }, [map_center, map_origin, camera_control_mode]);

  /** Resets camera distance to the configured default. */
  useEffect(() => {
    if (reset_signal.dist === 0) return;
    set_view_state({ dist: DEFAULT_DIST });
  }, [reset_signal.dist]);

  /** Resets camera polar angle to the configured default. */
  useEffect(() => {
    if (reset_signal.orientation === 0) return;
    set_view_state({ polar_angle: DEFAULT_POLAR_ANGLE });
  }, [reset_signal.orientation]);

  return { fly_target };
}

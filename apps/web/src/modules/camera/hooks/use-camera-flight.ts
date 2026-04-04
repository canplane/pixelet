// @pxl/web/modules/camera/hooks/use-camera-flight.ts

import { type RefObject, useEffect, useRef } from "react";
import * as THREE from "three";
import { useThree } from "@react-three/fiber";
import { OrbitControls as OrbitControlsImpl } from "three-stdlib";

import type { f64 } from "@xpute/spec/lib/kit/word.ts";
import type { IVec } from "@pxl/spec/space/pvf3/mod.ts";

import { useMapStore } from "@/modules/map/store.ts";
import { CameraStatus, useCameraStore } from "../store.ts";
import { type CameraFlyTarget, derive_camera_pose, type ViewState } from "../lib/pose.ts";
import { camera_cfg } from "../config.ts";

const { DEFAULT_DIST, MIN_DIST, DEFAULT_POLAR_ANGLE, TRAVEL_DIST_THRES } = camera_cfg;

type UseCameraFlightProps = {
  controls_ref: RefObject<OrbitControlsImpl | null>;
  set_is_flying: (is_flying: boolean) => void;
  map_origin: IVec;
};

export function useCameraFlight(props: UseCameraFlightProps) {
  const { controls_ref, set_is_flying, map_origin } = props;

  const map_center = useMapStore((state) => state.map_center);

  const view_state = useCameraStore((state) => state.view_state);
  const set_view_state = useCameraStore.getState().set_view_state;
  const reset_signal = useCameraStore((state) => state.camera_reset_signal);
  const set_camera_status = useCameraStore.getState().set_camera_status;

  const { camera } = useThree();

  const arrived_key = useRef("");
  const fly_target = useRef<CameraFlyTarget>({
    dst_pos: new THREE.Vector3(),
    dst_look_at: new THREE.Vector3(),
  });

  const dst_pos_reg = useRef(new THREE.Vector3());
  const dst_look_at_reg = useRef(new THREE.Vector3());

  function plan_flight(dst_pos: THREE.Vector3, dst_look_at: THREE.Vector3, opts: { snap?: boolean } = {}) {
    const controls: OrbitControlsImpl | null = controls_ref.current;
    if (!controls) return;

    const { snap = false } = opts;
    const pos_dist: f64 = controls.object.position.distanceTo(dst_pos);
    const look_at_dist: f64 = controls.target.distanceTo(dst_look_at);

    if (pos_dist < 0.01 && look_at_dist < 0.01) {
      set_is_flying(false);
      set_camera_status(CameraStatus.IDLE);
      return;
    }

    if (snap) {
      camera.position.copy(dst_pos);
      controls.target.copy(dst_look_at);
      controls.update();
      set_is_flying(false);
      set_camera_status(CameraStatus.IDLE);
      return;
    }

    fly_target.current.dst_pos.copy(dst_pos);
    fly_target.current.dst_look_at.copy(dst_look_at);
    set_camera_status(CameraStatus.FLYING);
    set_is_flying(true);
  }

  function plan_from_view(center: typeof map_center, origin: IVec, view: ViewState, opts: { snap?: boolean } = {}) {
    const controls: OrbitControlsImpl | null = controls_ref.current;
    if (!center || !controls) return;

    const dist: f64 = view.dist > MIN_DIST ? view.dist : DEFAULT_DIST;
    const polar_angle: f64 = view.polar_angle || DEFAULT_POLAR_ANGLE;

    const { look_at, pos } = derive_camera_pose(center, origin, dist, view.bearing, polar_angle);

    const dst_pos = dst_pos_reg.current.set(pos[0], pos[1], pos[2]);
    const dst_look_at = dst_look_at_reg.current.set(look_at[0], look_at[1], look_at[2]);

    const look_at_dist: f64 = controls.target.distanceTo(dst_look_at);
    const pos_dist: f64 = controls.object.position.distanceTo(dst_pos);
    const travel_dist: f64 = Math.max(look_at_dist, pos_dist);

    plan_flight(dst_pos, dst_look_at, {
      snap: opts.snap ?? (travel_dist > TRAVEL_DIST_THRES),
    });
  }

  useEffect((): void => {
    const controls: OrbitControlsImpl | null = controls_ref.current;
    if (!map_center || !controls) return;
    if (useCameraStore.getState().camera_status === CameraStatus.DRAG) return;

    const bound_key = [
      map_center.i.toString(),
      map_center.f.toString(),
      map_origin.toString(),
    ].join("|");
    if (arrived_key.current === bound_key) return;

    plan_from_view(map_center, map_origin, view_state, {
      snap: arrived_key.current === "",
    });

    arrived_key.current = bound_key;
  }, [map_center, map_origin]);

  useEffect(() => {
    if (reset_signal.dist === 0) return;

    const next_view: ViewState = {
      ...useCameraStore.getState().view_state,
      dist: DEFAULT_DIST,
    };

    set_view_state({ dist: DEFAULT_DIST });
    plan_from_view(useMapStore.getState().map_center, map_origin, next_view);
  }, [reset_signal.dist, map_origin]);

  useEffect(() => {
    if (reset_signal.orientation === 0) return;

    const next_view: ViewState = {
      ...useCameraStore.getState().view_state,
      bearing: 0,
      polar_angle: DEFAULT_POLAR_ANGLE,
    };

    set_view_state({
      bearing: 0,
      polar_angle: DEFAULT_POLAR_ANGLE,
    });
    plan_from_view(useMapStore.getState().map_center, map_origin, next_view);
  }, [reset_signal.orientation, map_origin]);

  return { fly_target };
}

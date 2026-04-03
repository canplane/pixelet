// @pxl/web/modules/camera/hooks/use-camera-frame.ts

import { type RefObject, useRef } from "react";
import { useFrame, useThree } from "@react-three/fiber";
import { OrbitControls as OrbitControlsImpl } from "three-stdlib";

import type { f64 } from "@xpute/spec/lib/kit/word.ts";

import { CameraStatus, useCameraStore } from "../store.ts";
import type { CameraFlyTarget } from "../lib/pose.ts";
import {
  calc_camera_view_sample,
  type CameraChangeSample,
  type CameraViewSample,
  DEFAULT_CAMERA_CHANGE_THRESHOLD,
  is_camera_changed,
} from "../lib/camera-change.ts";

type UseCameraFrameProps = {
  controls_ref: RefObject<OrbitControlsImpl | null>;
  fly_target: RefObject<CameraFlyTarget>;
  is_flying_ref: RefObject<boolean>;
  set_is_flying: (is_flying: boolean) => void;
};
export function useCameraFrame(props: UseCameraFrameProps) {
  const { controls_ref, fly_target, is_flying_ref, set_is_flying } = props;

  const set_view_state = useCameraStore.getState().set_view_state;
  const set_camera_status = useCameraStore.getState().set_camera_status;

  const { camera } = useThree();

  const last_update = useRef<CameraChangeSample>({
    target_x: 0,
    target_y: 0,
    target_z: 0,
    dist: 0,
    bearing: 0,
    polar_angle: 0,
  });
  const last_update_time = useRef(0);

  function sample_camera_change(controls: OrbitControlsImpl): CameraViewSample {
    return calc_camera_view_sample(
      camera.position.x,
      camera.position.y,
      camera.position.z,
      controls.target.x,
      controls.target.y,
      controls.target.z,
      controls.getPolarAngle(),
      controls.getAzimuthalAngle(),
    );
  }

  function mark_interaction_start() {
    const controls: OrbitControlsImpl | null = controls_ref.current;
    if (!controls) return null;
    return sample_camera_change(controls);
  }

  function handle_change() {
    const controls: OrbitControlsImpl | null = controls_ref.current;
    if (!controls) return;

    const cur: CameraViewSample = sample_camera_change(controls);

    const now: f64 = performance.now();
    if (now - last_update_time.current < 16) return;

    if (!is_camera_changed(cur, last_update.current, DEFAULT_CAMERA_CHANGE_THRESHOLD)) return;

    last_update.current = cur;
    last_update_time.current = now;
    set_view_state(cur);
  }

  useFrame((_, delta) => {
    const controls: OrbitControlsImpl | null = controls_ref.current;
    if (!controls || !is_flying_ref.current) return;

    const { dst_pos, dst_look_at } = fly_target.current;

    const alpha: f64 = 1 - Math.pow(0.001, delta);

    camera.position.lerp(dst_pos, alpha);
    controls.target.lerp(dst_look_at, alpha);

    controls.update();

    if (camera.position.distanceTo(dst_pos) < 0.05) {
      camera.position.copy(dst_pos);
      controls.target.copy(dst_look_at);
      controls.update();
      set_is_flying(false);
      set_camera_status(CameraStatus.IDLE);
    }
  });

  return { sample_camera_change, mark_interaction_start, handle_change };
}

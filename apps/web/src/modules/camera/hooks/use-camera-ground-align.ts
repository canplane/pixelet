// @pxl/web/modules/camera/hooks/use-camera-ground-align.ts

import { type RefObject, useLayoutEffect } from "react";
import { useThree } from "@react-three/fiber";
import { OrbitControls as OrbitControlsImpl } from "three-stdlib";

import type { f64 } from "@xpute/spec/lib/kit/word.ts";

import { ivec } from "@pxl/spec/space/pvf3/mod.ts";

import { useMapStore } from "@/modules/map/store.ts";
import { useMapNavigation } from "@/modules/map/navigation/use-map-navigation.ts";
import { get_ele } from "@/modules/world/terrain/use-elevation.ts";
import type { CameraFlyTarget } from "../lib/pose.ts";

/**
 * Aligns the camera target height to terrain whenever the canonical center changes.
 *
 * This avoids per-frame polling and only reacts when center ownership changes.
 */

type UseCameraGroundAlignProps = {
  controls_ref: RefObject<OrbitControlsImpl | null>;
  fly_target: RefObject<CameraFlyTarget>;
  is_flying_ref: RefObject<boolean>;
};
export function useCameraGroundAlign(props: UseCameraGroundAlignProps) {
  const { controls_ref, fly_target, is_flying_ref } = props;

  const map_center = useMapStore((state) => state.map_center);
  const map_origin = useMapNavigation().map_origin;

  const { camera } = useThree();

  useLayoutEffect(() => {
    const controls = controls_ref.current;
    if (!controls || map_center === null) return;

    const center_ele = get_ele(map_center.i, map_center.f);
    if (center_ele == null) return;

    const origin_z: f64 = ivec.to_real(ivec.Z(map_origin));
    const target_z: f64 = center_ele - origin_z;

    const current_z: f64 = controls.target.z;

    const delta_z: f64 = target_z - current_z;

    if (Math.abs(delta_z) < 0.01) return;

    if (is_flying_ref.current) {
      /** Preserve the current path and only retarget the landing height. */
      fly_target.current.dst_look_at.z = target_z;
      fly_target.current.dst_pos.z += delta_z;
    } else {
      /** Apply the terrain-aligned height immediately while idle/snapped. */
      camera.position.z += delta_z;
      controls.target.z = target_z;

      controls.update();
    }
  }, [map_center, map_origin, camera, fly_target, is_flying_ref, controls_ref]);
}

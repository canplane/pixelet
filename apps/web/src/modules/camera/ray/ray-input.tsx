// @pxl/web/modules/camera/ray/ray-input.tsx

import type { ThreeEvent } from "@react-three/fiber";
import { useThree } from "@react-three/fiber";

import type { f64 } from "@xpute/spec/lib/kit/word.ts";

import { type IVec, type PVF3, pvf3 } from "@pxl/spec/space/pvf3/mod.ts";

import { get_ele, with_ele } from "@/modules/world/terrain/use-elevation.ts";
import { CameraStatus, useCameraStore } from "../store.ts";
import { ray_hit_terrain } from "./ray.ts";
import { U_PER_CELL } from "@pxl/bake/terrain/constants.ts";

const INTERACT_RADIUS_CELL: f64 = 32 / U_PER_CELL - 0.707;
const INTERACT_RADIUS_U: f64 = INTERACT_RADIUS_CELL * U_PER_CELL;
const INTERACT_RADIUS2_U: f64 = INTERACT_RADIUS_U * INTERACT_RADIUS_U;

function in_interact_radius(a: PVF3, b: PVF3): boolean {
  const dx: f64 = pvf3.to_real(pvf3.X(a)) - pvf3.to_real(pvf3.X(b));
  const dy: f64 = pvf3.to_real(pvf3.Y(a)) - pvf3.to_real(pvf3.Y(b));
  return dx * dx + dy * dy <= INTERACT_RADIUS2_U;
}

type CameraRayInputProps = {
  map_center: PVF3 | null;
  map_origin: IVec;
};
export function CameraRayInput(props: CameraRayInputProps) {
  const { map_center, map_origin } = props;

  const set_hovered_coord = useCameraStore.getState().set_hovered_coord;
  const clear_hovered_coord = useCameraStore.getState().clear_hovered_coord;

  const { raycaster, pointer, camera } = useThree();

  function handle_pointer_move(e: ThreeEvent<MouseEvent>) {
    e.stopPropagation();

    if (useCameraStore.getState().camera_status !== CameraStatus.IDLE) {
      clear_hovered_coord();
      return;
    }

    raycaster.setFromCamera(pointer, camera);
    const hit: PVF3 | null = ray_hit_terrain(raycaster.ray, map_origin, get_ele, with_ele);
    if (!hit || !map_center) {
      clear_hovered_coord();
      return;
    }

    if (!in_interact_radius(hit, map_center)) {
      clear_hovered_coord();
      return;
    }

    set_hovered_coord(hit);
  }

  return (
    <mesh
      onPointerMove={handle_pointer_move}
      onPointerOut={clear_hovered_coord}
    >
      <planeGeometry args={[1e5, 1e5]} />
      <meshBasicMaterial transparent opacity={0} depthWrite={false} />
    </mesh>
  );
}

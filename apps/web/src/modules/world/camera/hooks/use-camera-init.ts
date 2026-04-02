// @pxl/web/modules/world/camera/hooks/use-camera-init.ts

import { useEffect, useLayoutEffect } from "react";
import { useThree } from "@react-three/fiber";

import { useMapStore } from "@/modules/map/navigation/map-store.ts";
import { useMapNavigation } from "@/modules/map/navigation/use-map-navigation.ts";
import { render_cfg } from "../../render/config.ts";
import { CAMERA_UP } from "../../render/camera.ts";

const { FAR_CLIP } = render_cfg;

export function useCameraInit() {
  const map_center = useMapStore((state) => state.map_center);
  const set_map_center = useMapStore.getState().set_map_center;

  const device_coord = useMapNavigation().device_coord;

  const { camera } = useThree();

  /** Applies one-time camera runtime defaults for the Z-up scene. */
  useEffect((): void => {
    camera.up.set(...CAMERA_UP);
    camera.far = FAR_CLIP;
    camera.updateProjectionMatrix();
  }, [camera]);

  /** Initializes the canonical map center from the device coordinate. */
  useLayoutEffect(() => {
    if (map_center === null && device_coord !== null) {
      set_map_center(device_coord);
    }
  }, [map_center, device_coord]);
}

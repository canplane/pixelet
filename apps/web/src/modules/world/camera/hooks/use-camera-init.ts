// @pxl/web/modules/world/camera/hooks/use-camera-init.ts

import { useEffect, useLayoutEffect } from "react";
import { useThree } from "@react-three/fiber";

import { render_cfg } from "@/modules/world/render/config.ts";
import { useMapStore } from "@/modules/map/navigation/map-store.ts";
import { useMapNavigation } from "@/modules/map/navigation/use-map-navigation.ts";

const { FAR_CLIP } = render_cfg.rendering;

export function useCameraInit() {
  const map_center = useMapStore((state) => state.map_center);
  const set_map_center = useMapStore.getState().set_map_center;

  const device_coord = useMapNavigation().device_coord;

  const { camera } = useThree();

  // 1. [Camera-Config] 카메라 투영 행렬 설정
  useEffect((): void => {
    camera.far = FAR_CLIP;
    camera.updateProjectionMatrix();
  }, [camera]);

  // 2. [Initial-Trigger] 세상의 첫 위치 결정
  useLayoutEffect(() => {
    if (map_center === null && device_coord) {
      set_map_center(device_coord);
    }
  }, [map_center, device_coord]);
}

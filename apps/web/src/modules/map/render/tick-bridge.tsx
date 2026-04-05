// @pxl/web/modules/map/render/tick-bridge.tsx

import { useEffect, useRef } from "react";
import { useFrame, useThree } from "@react-three/fiber";
import * as THREE from "three";

import { useMapStore } from "@/modules/map/store.ts";
import { useFrameRefs } from "@/modules/world/frame/context.tsx";
import { get_local_world_origin } from "@/modules/world/frame/coord.ts";

import { RenderTickController } from "./tick-controller.ts";

/**
 * Render tick bridge.
 *
 * ROLE:
 * - minimal R3F hook entry point
 * - forwards per-frame runtime state into RenderTickController
 *
 * NOTES:
 * - React should not own render math
 * - this component exists only to bridge R3F clock/camera access
 */
export function RenderTickBridge() {
  const { camera } = useThree();
  const { world_frame_ref, local_world_frame_ref } = useFrameRefs();

  const ctl_ref = useRef<RenderTickController | null>(null);

  useEffect(() => {
    if (!(camera instanceof THREE.PerspectiveCamera)) {
      throw new Error("RenderTickBridge requires THREE.PerspectiveCamera");
    }

    const ctl = new RenderTickController();
    ctl.init_camera(camera);
    ctl_ref.current = ctl;

    return () => {
      ctl_ref.current = null;
    };
  }, [camera]);

  useFrame((_, delta) => {
    const ctl = ctl_ref.current;
    if (!ctl) return;

    if (!(camera instanceof THREE.PerspectiveCamera)) return;

    const map_center = useMapStore.getState().map_center;
    const map_origin = get_local_world_origin(map_center);

    ctl.tick({
      camera,
      world_frame: world_frame_ref.current,
      local_world_frame: local_world_frame_ref.current,
      map_center,
      map_origin,
      delta,
    });
  });

  return null;
}

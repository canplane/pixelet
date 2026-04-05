// @pxl/web/modules/map/render/root.tsx

import { useRef } from "react";
import * as THREE from "three";

import { CameraInput } from "@/modules/camera/input.tsx";
import { FrameProvider } from "@/modules/world/frame/context.tsx";

import { MapEnvironment } from "../environment.tsx";
import { World } from "@/modules/world/world.tsx";
import { RenderTickBridge } from "./tick-bridge.tsx";

/**
 * R3F scene root content.
 *
 * ROLE:
 * - pure scene graph only
 * - no canvas ownership
 * - no renderer bootstrap
 * - no DOM sizing / lifecycle wiring
 */
export function MapRenderRoot() {
  const world_ref = useRef<THREE.Group>(null);

  return (
    <FrameProvider>
      <RenderTickBridge />
      <CameraInput />

      {/* Lighting and sky environment */}
      {/* <MapEnvironment /> */}

      {/* World layers in the floating-origin local frame */}
      <World ref={world_ref} />
    </FrameProvider>
  );
}

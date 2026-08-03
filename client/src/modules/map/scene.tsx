// @pxl/web/modules/map/scene.tsx

import { useRef } from "react";
import { Canvas } from "@react-three/fiber";
import * as THREE from "three";
import { EffectComposer, SMAA } from "@react-three/postprocessing";

import { useCamera } from "@/modules/camera/use-camera.ts";
import { CameraInput } from "@/modules/camera/input.tsx";
import { camera_cfg } from "@/modules/camera/config.ts";
import { render_cfg } from "@/modules/world/render/config.ts";
import { get_theme_key, themes, type WorldTheme } from "@/modules/world/render/themes.ts";
import { FrameProvider } from "@/modules/world/frame/context.tsx";

import { MapEnvironment } from "./environment.tsx";
import { World } from "@/modules/world/world.tsx";
import { useMapBootstrap } from "./navigation/use-map-bootstrap.ts";
import { useTileFetcher } from "@/modules/world/terrain/use-tile-fetcher.ts";
import { MapOverlays } from "./ui/overlay/map-overlays.tsx";
import { MapHTMLPortal } from "./portal/html-portal.tsx";

import { MapToastController } from "./ui/toast/toast-controller.tsx";

const { FOV, NEAR } = camera_cfg.projection;
const { FAR_CLIP } = render_cfg;

function CameraRig() {
  useCamera();
  return <CameraInput />;
}

export function MapScene() {
  useMapBootstrap();
  useTileFetcher();

  const world_ref = useRef<THREE.Group>(null);

  const theme_key = get_theme_key();
  const theme: WorldTheme = themes[theme_key];

  return (
    <div className="fixed inset-0 overflow-hidden bg-black">
      <MapToastController />

      {/* Theme background layer matched to fog color to hide the 2D/3D seam. */}
      <div
        className="absolute inset-0 transition-colors duration-1500 ease-in-out"
        style={{
          backgroundColor: theme.fog,
        }}
      >
        <Canvas
          dpr={1}
          gl={{
            antialias: false,
            powerPreference: "high-performance",
            alpha: false,
            stencil: false,
            depth: true,
          }}
          shadows
          camera={{
            fov: FOV,
            near: NEAR,
            far: FAR_CLIP,
          }}
          frameloop="always"
        >
          <FrameProvider>
            <CameraRig />

            {/* Lighting and sky environment */}
            <MapEnvironment />

            {/* World layers in the floating-origin local frame */}
            <World ref={world_ref} />
          </FrameProvider>

          {/* Post AA for voxel edge cleanup */}
          <EffectComposer multisampling={0}>
            <SMAA />
          </EffectComposer>
        </Canvas>
      </div>

      {/* HTML overlay portal */}
      <MapHTMLPortal />

      {/* Absolute-position UI layer */}
      <MapOverlays />
    </div>
  );
}

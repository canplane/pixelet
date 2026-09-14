// @pxl/web/module/map/scene.tsx

import { Canvas } from "@react-three/fiber";

import { useCamera } from "@/module/camera/use_camera.ts";
import { CameraInput } from "@/module/camera/input.tsx";
import { camera_cfg } from "@/module/camera/config.ts";
import { post_cfg, render_cfg } from "@/module/world/environment/config.ts";
import { themes, type WorldTheme } from "@/module/world/environment/theme.ts";
import { useThemeKey } from "@/module/world/environment/theme_clock.ts";

import { MapEnvironment } from "./environment.tsx";
import { TickDriver } from "@/module/world/sched/tick.ts";
import { World } from "@/module/world/world.tsx";
import { useMapBootstrap } from "./navigation/use_map_bootstrap.ts";
import { useTileFetcher } from "@/module/world/terrain/use_tile_fetcher.ts";
import { usePoiFetcher } from "@/module/world/poi/use_poi_fetcher.ts";
import { useGroundSnapSync } from "@/module/world/terrain/use_ground_snap_sync.ts";
import { Overlays } from "./ui/overlay/overlays.tsx";
import { MapHTMLPortal } from "./portal/html_portal.tsx";

import { GpuCanvas } from "@/render/gpu/stage.tsx";
import { ToastController } from "./ui/toast/toast_controller.tsx";
import { LedgerOverlay } from "@/module/world/sched/ledger_overlay.tsx";

const { FOV, NEAR } = camera_cfg.projection;
const { FAR_CLIP } = render_cfg;

function CameraRig() {
  useCamera();
  return <CameraInput />;
}

export function MapScene() {
  useMapBootstrap();
  useTileFetcher();
  usePoiFetcher();
  useGroundSnapSync();

  const theme_key = useThemeKey();
  const theme: WorldTheme = themes[theme_key];

  return (
    <div className="fixed inset-0 overflow-hidden bg-black">
      <ToastController />
      <LedgerOverlay />

      {/* Theme background layer matched to fog color to hide the 2D/3D seam. */}
      <div
        className="absolute inset-0 transition-colors duration-1500 ease-in-out"
        style={{
          backgroundColor: theme.fog,
        }}
      >
        {
          /* The WebGPU renderer's canvas, where everything is drawn; three's
            above it draws only the dev gizmos until three goes. */
        }
        <GpuCanvas />
        <Canvas
          style={{ position: "relative", zIndex: 1 }}
          // The device's own pixel ratio, capped at 2 (post_cfg.DPR): the
          // resolution every other layer of the page is drawn at on that
          // device, and the cap is where the saving is — a 3x/4x phone would
          // otherwise pay nine or sixteen times the fragment work of 1x for a
          // difference the eye does not see at that density.
          dpr={post_cfg.DPR}
          gl={{
            antialias: false,
            powerPreference: "high-performance",
            // Transparent where nothing of three's is drawn, so the WebGPU
            // canvas under it shows through.
            alpha: true,
            stencil: false,
            depth: true,
          }}
          camera={{
            fov: FOV,
            near: NEAR,
            far: FAR_CLIP,
          }}
          // "always" rendered every frame unconditionally, even fully at
          // rest. "demand" only renders when something calls invalidate() —
          // every useFrame that changes something over time now has to ask
          // for its own next frame explicitly: use_camera.ts (camera
          // damping), use_frame_translation.ts/use_frame_rotation.ts
          // (floating-origin frame damping), environment.tsx (real-time
          // sun/moon + theme color lerps, via a plain interval since
          // there's no prop/store change to hang a wake-up effect off of),
          // and location_puck.tsx's beacon spin/bob (which never converges
          // by design, so it just keeps re-invalidating for as long as it's
          // visible — see its own comment for why that puts a floor under
          // how idle this can ever get). chunk_stage.tsx additionally
          // invalidates while its global reveal/LOD-remesh budgets still
          // have queued work, then becomes camera-driven again once those
          // queues drain.
          frameloop="demand"
        >
          {
            /* The one useFrame in this Canvas. Every per-frame subscriber in
              the world registers a step into a phase of this tick instead
              (sched/tick.ts) — one order, one budget. */
          }
          <TickDriver />
          <CameraRig />

          {/* Lighting and sky environment */}
          <MapEnvironment />

          {/* World layers in the root frame */}
          <World />
        </Canvas>
      </div>

      {/* HTML overlay portal */}
      <MapHTMLPortal />

      {/* Absolute-position UI layer */}
      <Overlays />
    </div>
  );
}

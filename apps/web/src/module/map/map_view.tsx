// @pxl/web/module/map/map_view.tsx

/**
 * The map as the page shows it: the tick driver over the WebGPU canvas, the
 * camera and its input, the world, the fetchers that follow the view, and
 * the overlays on top.
 */

import { useState } from "react";

import { useCamera } from "@/module/camera/use_camera.ts";
import { CameraInput } from "@/module/camera/input.tsx";
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

import { GpuCanvas } from "@/render/gpu/stage.tsx";
import { ToastController } from "./ui/toast/toast_controller.tsx";
import { LedgerOverlay } from "@/module/world/sched/ledger_overlay.tsx";

function CameraRig({ element }: { element: HTMLCanvasElement | null }) {
  useCamera();
  return <CameraInput element={element} />;
}

export function MapView() {
  useMapBootstrap();
  useTileFetcher();
  usePoiFetcher();
  useGroundSnapSync();

  const theme_key = useThemeKey();
  const theme: WorldTheme = themes[theme_key];
  /** The renderer's canvas, once mounted: the frame loop's viewport and the
   * pointer's target. */
  const [canvas, set_canvas] = useState<HTMLCanvasElement | null>(null);

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
        <GpuCanvas on_canvas={set_canvas} />
        {
          /* The one frame loop. Every per-frame subscriber in the world
            registers a step into a phase of this tick (sched/tick.ts) —
            one order, one budget — and the components below render
            nothing themselves: they tell the module and the renderer. */
        }
        <TickDriver element={canvas} />
        <CameraRig element={canvas} />

        {/* Lighting and sky environment */}
        <MapEnvironment />

        {/* World layers in the root frame */}
        <World />
      </div>

      {/* HTML overlay portal */}

      {/* Absolute-position UI layer */}
      <Overlays />
    </div>
  );
}

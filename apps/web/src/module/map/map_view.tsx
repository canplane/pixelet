// @pxl/web/module/map/map_view.tsx

/**
 * The map as the page shows it: the kernel's frame loop over the WebGPU
 * canvas, and the overlays on top. The camera and its input, the world, the
 * environment and the fetches that follow the view are the kernel's turn
 * (wasm/kernel.ts); this renders the canvas and the chrome.
 */

import { useCallback, useState } from "react";

import { KernelDriver, request_frame, useKernel } from "@/wasm/kernel.ts";
import { GpuCanvas } from "@/render/gpu/stage.tsx";
import { LedgerOverlay } from "@/module/world/sched/ledger_overlay.tsx";

import { Overlays } from "./ui/overlay/overlays.tsx";
import { ToastController } from "./ui/toast/toast_controller.tsx";

export function MapView() {
  const fog = useKernel((s) => s.fog);
  const msaa = useKernel((s) => s.settings.msaa === true);
  /** The renderer's canvas, once mounted: the frame loop's viewport and the
   * pointer's target. */
  const [canvas, set_canvas] = useState<HTMLCanvasElement | null>(null);
  const on_ready = useCallback(() => request_frame(), []);

  return (
    <div className="fixed inset-0 overflow-hidden bg-black">
      <ToastController />
      <LedgerOverlay />

      {/* Theme background layer matched to fog color to hide the 2D/3D seam. */}
      <div className="absolute inset-0 transition-colors duration-1500 ease-in-out" style={{ backgroundColor: fog }}>
        <GpuCanvas on_canvas={set_canvas} on_ready={on_ready} msaa={msaa} />
        <KernelDriver element={canvas} />
      </div>

      {/* Absolute-position UI layer */}
      <Overlays />
    </div>
  );
}

// @pxl/web/feature/map/ui/view.tsx

/**
 * The map as the page shows it: the world's canvas, handed to the kernel,
 * and the overlays on top. The camera and its input, the world, the
 * environment, the fetches that follow the view and the drawing are the
 * kernel's turn (kernel/kernel.ts); this lays out the canvas and the chrome.
 */

import { useEffect, useRef, useState } from "react";

import { KernelDriver } from "@/kernel/control/input.ts";
import { open_surface, SURFACE_WORLD } from "@/kernel/render/surface.ts";
import { LedgerOverlay } from "@/feature/dev/ledger/overlay.tsx";

import { Overlays } from "./overlay/overlays.tsx";
import { ToastController } from "./toast-controller.tsx";

export function MapView() {
  const ref = useRef<HTMLCanvasElement>(null);
  /** The world's canvas once the kernel has it: the frame loop's viewport and
   * the pointer's target. */
  const [canvas, setCanvas] = useState<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    const surface = open_surface(element, SURFACE_WORLD);
    setCanvas(element);
    return () => {
      setCanvas(null);
      surface.close();
    };
  }, []);

  return (
    <div className="fixed inset-0 overflow-hidden bg-black">
      <ToastController />
      <LedgerOverlay />

      <div className="absolute inset-0">
        <canvas ref={ref} className="absolute inset-0 h-full w-full touch-none" />
        <KernelDriver element={canvas} />
      </div>

      {/* Absolute-position UI layer */}
      <Overlays />
    </div>
  );
}

// @pxl/web/modules/map/render/host.tsx

import { useEffect, useRef } from "react";

import { send_render_canvas } from "@/xpute/render_bridge.ts";

export function MapCanvasHost() {
  const host_ref = useRef<HTMLDivElement>(null);
  const canvas_ref = useRef<HTMLCanvasElement>(null);
  const transferred_ref = useRef(false);

  useEffect(() => {
    const canvas = canvas_ref.current!;
    if (!canvas) return;
    if (transferred_ref.current) return; // StrictMode double-invoke guard
    transferred_ref.current = true;

    // Transfer control to the CORE Worker (Rust engine).
    // Once transferred, this thread can no longer draw to the canvas.
    const offscreen = canvas.transferControlToOffscreen();
    send_render_canvas(offscreen);
  }, []);

  return (
    <div ref={host_ref} className="absolute inset-0">
      <canvas ref={canvas_ref} className="block h-full w-full" />
    </div>
  );
}

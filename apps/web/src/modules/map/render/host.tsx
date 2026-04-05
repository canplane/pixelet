// @pxl/web/modules/map/render/host.tsx

import { useEffect, useRef } from "react";
import { createRoot, events, type ReconcilerRoot } from "@react-three/fiber";

import { camera_cfg } from "@/modules/camera/config.ts";
import { render_cfg } from "@/modules/world/render/config.ts";

import { create_map_renderer } from "./bootstrap.ts";
import { MapRenderRoot } from "./root.tsx";

const { FOV, NEAR } = camera_cfg.projection;
const { FAR_CLIP } = render_cfg;

type Size = {
  width: number;
  height: number;
  top: number;
  left: number;
};

export function MapCanvasHost() {
  const host_ref = useRef<HTMLDivElement>(null);
  const canvas_ref = useRef<HTMLCanvasElement>(null);
  const root_ref = useRef<ReconcilerRoot<HTMLCanvasElement> | null>(null);

  useEffect(() => {
    const host = host_ref.current;
    const canvas = canvas_ref.current;
    if (!host || !canvas) return;

    const host_el = host;
    const canvas_el = canvas;

    const root = createRoot(canvas_el);
    root_ref.current = root;

    let did_dispose = false;
    let did_configure = false;
    let resize_queued = false;

    function get_size(): Size {
      const rect = host_el.getBoundingClientRect();
      return {
        width: Math.max(1, rect.width | 0),
        height: Math.max(1, rect.height | 0),
        top: rect.top,
        left: rect.left,
      };
    }

    async function set_size(): Promise<void> {
      if (did_dispose || !did_configure || resize_queued) return;
      resize_queued = true;
      try {
        await root.configure({
          size: get_size(),
        });
      } finally {
        resize_queued = false;
      }
    }

    async function boot(): Promise<void> {
      await root.configure({
        events,
        shadows: true,
        frameloop: "always",
        dpr: 1,
        size: get_size(),
        gl: create_map_renderer,
        camera: {
          fov: FOV,
          near: NEAR,
          far: FAR_CLIP,
        },
        onCreated: (state) => {
          state.events?.connect?.(host_el);
        },
      });

      if (did_dispose) return;

      did_configure = true;
      root.render(<MapRenderRoot />);

      const ro = new ResizeObserver(() => {
        void set_size();
      });

      ro.observe(host_el);

      cleanup = () => {
        ro.disconnect();
      };
    }

    let cleanup = () => {};

    void boot();

    return () => {
      did_dispose = true;
      cleanup();

      const root = root_ref.current;
      root_ref.current = null;
      root?.unmount();
    };
  }, []);

  return (
    <div ref={host_ref} className="absolute inset-0">
      <canvas ref={canvas_ref} className="block h-full w-full" />
    </div>
  );
}

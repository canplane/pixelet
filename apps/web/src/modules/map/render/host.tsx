// @pxl/web/modules/map/render/host.tsx

import { useEffect, useRef } from "react";

import type { u32 } from "@xpute/spec/lib/kit/word.ts";

import { install_camera_input } from "@/modules/camera/input.tsx";

import type { RenderSurface } from "@/render/types.ts";
import { get_gpu_device, set_render_camera_state, set_render_surface, set_render_world_state } from "@/render/runtime.ts";
import { FramePolicy } from "@/render/frame/policy.ts";
import { dispose_world_renderer, draw_world_renderer, init_world_renderer, type WorldRenderer } from "@/render/world_renderer.ts";

type Size = {
  width: u32;
  height: u32;
};

export function MapCanvasHost() {
  const host_ref = useRef<HTMLDivElement>(null);
  const canvas_ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const host = host_ref.current!; // effect runs after mount
    const canvas = canvas_ref.current!; // effect runs after mount
    if (!host || !canvas) return;

    const device = get_gpu_device();
    const context = canvas.getContext("webgpu")!; // keep assertion because TS narrowing is weak here
    if (!context) throw new Error("web: failed to acquire WebGPU canvas context");

    const format = navigator.gpu.getPreferredCanvasFormat();

    let disposed = false;
    let raf_id = 0;
    let prev_time = 0;

    const surface: RenderSurface = {
      canvas,
      context,
      format,
      width: 1,
      height: 1,
    };

    const frame_policy = new FramePolicy();
    const world_renderer: WorldRenderer = init_world_renderer(device, format);

    const dispose_input = install_camera_input({
      canvas,
      get_world_azimuth: () => frame_policy.get_world_azimuth(),
    });

    function get_size(): Size {
      const rect = host.getBoundingClientRect();
      return {
        width: Math.max(1, Math.floor(rect.width)),
        height: Math.max(1, Math.floor(rect.height)),
      };
    }

    function apply_size(): void {
      const size = get_size();
      if (
        surface.width === size.width &&
        surface.height === size.height &&
        canvas.width === size.width &&
        canvas.height === size.height
      ) {
        return;
      }

      surface.width = size.width;
      surface.height = size.height;

      canvas.width = size.width;
      canvas.height = size.height;

      context.configure({
        device,
        format,
        alphaMode: "opaque",
      });

      set_render_surface(surface);
    }

    function frame(time_ms: number): void {
      if (disposed) return;

      const time = time_ms * 0.001;
      const dt = prev_time === 0 ? 0 : time - prev_time;
      prev_time = time;

      const out = frame_policy.tick({
        width: surface.width,
        height: surface.height,
        dt,
      });

      set_render_camera_state(out.camera);
      set_render_world_state(out.world);

      draw_world_renderer(world_renderer, {
        device,
        surface,
        time,
        dt,
      });

      raf_id = requestAnimationFrame(frame);
    }

    apply_size();
    set_render_surface(surface);

    const ro = new ResizeObserver(() => {
      apply_size();
    });

    ro.observe(host);
    raf_id = requestAnimationFrame(frame);

    return () => {
      disposed = true;
      cancelAnimationFrame(raf_id);
      ro.disconnect();

      dispose_input();
      dispose_world_renderer(world_renderer);

      set_render_surface(null);
      set_render_camera_state(null);
      set_render_world_state(null);
    };
  }, []);

  return (
    <div ref={host_ref} className="absolute inset-0">
      <canvas ref={canvas_ref} className="block h-full w-full" />
    </div>
  );
}

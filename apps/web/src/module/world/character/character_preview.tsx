// @pxl/web/module/world/character/character_preview.tsx

/**
 * Small display case for the variant picker — one character playing its idle
 * clip while slowly spinning in place, so a pick can actually be seen before
 * committing to it. The case's actor, its model and its spin are the kernel's
 * (crates/pxl/core/src/module/world/character/character_preview.rs); the
 * renderer draws it into this canvas alone each frame (render/gpu/preview.ts).
 */

import { useEffect, useRef, useState } from "react";

import { image_of, kernel_action, request_frame, useKernel } from "@/wasm/kernel.ts";
import { gpu_renderer } from "@/render/gpu/stage.tsx";
import type { CharacterVariant } from "@/module/world/character/variant.ts";

let next_case = 1;

export type CharacterPreviewProps = {
  variant: CharacterVariant;
  className?: string;
};

export function CharacterPreview(props: CharacterPreviewProps) {
  const { variant, className } = props;
  const ref = useRef<HTMLCanvasElement>(null);
  const [id] = useState(() => next_case++);
  const row = useKernel((s) => s.previews.find((p) => p.id === id) ?? null);
  const row_ref = useRef(row);
  row_ref.current = row;

  useEffect(() => {
    kernel_action({ type: "preview_open", id, key: variant.key });
    return () => {
      kernel_action({ type: "preview_close", id });
    };
  }, [id, variant.key]);

  useEffect(() => {
    const canvas = ref.current;
    const gpu = gpu_renderer;
    if (!canvas || !gpu) return;
    const preview = gpu.open_preview(canvas);
    let alive = true;
    let raf = 0;
    // The case draws on the page's frames: the kernel's tick runs only when
    // asked, and an open case is what asks.
    const frame = () => {
      if (!alive) return;
      const rect = canvas.getBoundingClientRect();
      const dpr = Math.min(2, globalThis.devicePixelRatio || 1);
      preview.resize(rect.width * dpr, rect.height * dpr);
      const r = row_ref.current;
      if (r) {
        preview.model = r.model;
        const skin = image_of(r.skin);
        if (skin) preview.set_skin(r.model, skin);
      }
      request_frame();
      raf = requestAnimationFrame(frame);
    };
    raf = requestAnimationFrame(frame);
    return () => {
      alive = false;
      cancelAnimationFrame(raf);
      gpu.close_preview(preview);
    };
  }, []);

  return (
    <div className={className}>
      <canvas ref={ref} className="h-full w-full" />
    </div>
  );
}

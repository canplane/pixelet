// @pxl/web/module/world/character/character_preview.tsx

/**
 * Small display case for the variant picker — one character playing its idle
 * clip while slowly spinning in place, so a pick can actually be seen before
 * committing to it. The canvas is handed to the kernel as a case; the actor,
 * its model, its spin and the drawing are the kernel's
 * (crates/pxl/core/src/module/world/character/character_preview.rs,
 * render/gpu/renderer.rs).
 */

import { useEffect, useRef } from "react";

import { kernel_action, open_surface, SURFACE_CASE } from "@/wasm/kernel.ts";
import type { CharacterVariant } from "@/module/world/character/variant.ts";

export type CharacterPreviewProps = {
  variant: CharacterVariant;
  className?: string;
};

export function CharacterPreview(props: CharacterPreviewProps) {
  const { variant, className } = props;
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const surface = open_surface(canvas, SURFACE_CASE);
    if (surface.handle !== 0) kernel_action({ type: "preview_open", id: surface.handle, key: variant.key });
    return () => {
      if (surface.handle !== 0) kernel_action({ type: "preview_close", id: surface.handle });
      surface.close();
    };
  }, [variant.key]);

  return (
    <div className={className}>
      <canvas ref={ref} className="h-full w-full" />
    </div>
  );
}

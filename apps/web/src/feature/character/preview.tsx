// @pxl/web/feature/character/preview.tsx

/**
 * Small display case for the variant picker — one character playing its idle
 * clip while slowly spinning in place, so a pick can actually be seen before
 * committing to it. The canvas is handed to the kernel as a case; the actor,
 * its model, its spin and the drawing are the kernel's
 * (crates/pxl/web/kernel/src/layer/character/preview.rs,
 * crates/pxl/web/render/src/gpu/renderer.rs).
 */

import { useEffect, useRef } from "react";

import { Command, encode_preview_create, encode_preview_delete } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command } from "@/kernel/shell.ts";
import { open_surface } from "@/kernel/render/surface.ts";
import { surface_role } from "@/kernel/render/gpu/api.spec.ts";
import type { CharacterVariant } from "@/feature/character/variant.ts";

export interface CharacterPreviewProps {
  variant: CharacterVariant;
  className?: string;
}

export function CharacterPreview(props: CharacterPreviewProps) {
  const { variant, className } = props;
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const surface = open_surface(canvas, surface_role.CASE);
    if (surface.handle !== 0) kernel_command(Command.PREVIEW_CREATE, encode_preview_create({ surface: surface.handle, group: variant.group, number: variant.number }));
    return () => {
      if (surface.handle !== 0) kernel_command(Command.PREVIEW_DELETE, encode_preview_delete({ surface: surface.handle }));
      surface.close();
    };
  }, [variant.group, variant.number]);

  return (
    <div className={className}>
      <canvas ref={ref} className="h-full w-full" />
    </div>
  );
}

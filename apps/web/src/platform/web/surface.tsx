// @pxl/web/platform/web/surface.tsx

/**
 * What the kernel draws into, on the web: a canvas handed over once, which
 * it draws through the host's WebGPU. A native build puts its own view here,
 * which the kernel draws into directly.
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import { useEffect, useRef, useState } from "react";

import { KernelDriver } from "@/kernel/control/input.ts";
import { open_surface } from "@/kernel/render/surface.ts";
import { surface_role } from "@/kernel/render/gpu/api.spec.ts";

const FILL = { position: "absolute", inset: 0, width: "100%", height: "100%" } as const;

/** The world: the map's surface, and the pointer driven from it. */
export function WorldSurface() {
  const ref = useRef<HTMLCanvasElement>(null);
  /** The canvas once the kernel has it: the frame loop's viewport and the
   * pointer's target. */
  const [canvas, setCanvas] = useState<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    const surface = open_surface(element, surface_role.WORLD);
    setCanvas(element);
    return () => {
      setCanvas(null);
      surface.close();
    };
  }, []);

  return (
    <>
      <canvas ref={ref} style={{ ...FILL, touchAction: "none" }} />
      <KernelDriver element={canvas} />
    </>
  );
}

/** A display case: a surface the kernel draws one thing into. `onOpen` is
 * handed the surface once it is the kernel's, and what it returns is run
 * before the surface is let go; it opens again whenever `openKey` changes. */
export function CaseSurface({ onOpen, openKey }: { onOpen: (surface: u32) => () => void; openKey: string }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const open = useRef(onOpen);
  open.current = onOpen;

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const surface = open_surface(canvas, surface_role.CASE);
    const close = surface.handle !== 0 ? open.current(surface.handle) : () => {};
    return () => {
      close();
      surface.close();
    };
  }, [openKey]);

  return <canvas ref={ref} style={FILL} />;
}

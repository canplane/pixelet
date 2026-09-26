// @pxl/web/platform/web/surface.tsx

/**
 * What the kernel draws into, on the web: a canvas handed over once, which it
 * draws through the page's WebGPU (webgpu.ts). A device puts its own view
 * here, which the kernel draws into directly.
 *
 * The world's canvas is also the page's driver (kernel/control/input.ts): its
 * box is the frame's viewport, its pointer events and the wheel over it are
 * the kernel's input, and the tab's visibility is the app's.
 */

import type { f64, u32 } from "@xpute/core/abi/word.ts";
import { round } from "@xpute/core/math/scalar.ts";
import { useEffect, useRef } from "react";

import { PointerKind, PointerPhase } from "@/kernel/abi/cmd.spec.ts";
import { kernel_open, kernel_pointer, kernel_rect, kernel_visible, kernel_wheel } from "@/kernel/control/input.ts";
import { surface_role } from "@/kernel/render/gpu/api.spec.ts";
import { surface_create, surface_delete, surface_size } from "@/kernel/render/surface.ts";
import { request_frame } from "@/kernel/sched.ts";
import { bind_surface, unbind_surface } from "@/platform/web/webgpu.ts";

const FILL = { position: "absolute", inset: 0, width: "100%", height: "100%" } as const;

function dpr(): f64 {
  return globalThis.devicePixelRatio || 1;
}

/**
 * Hands `canvas` to the kernel to draw `role` into, and follows its layout
 * box in device pixels: the handle, 0 when the kernel has no surface left and
 * nothing is drawn in it, and what lets it go.
 *
 * The box is the layout's, not the bounding rectangle: a rectangle includes
 * transforms, so a canvas mounted inside a drawer that is sliding or scaling in
 * would report a size it only passes through, and no resize follows a
 * transform. Where the observer reports device pixels they are exact; where it
 * does not (Safari), the CSS box times the pixel ratio stands in, and a zoom —
 * which changes the ratio and not the box — is caught by the window's resize.
 */
function open_surface(canvas: HTMLCanvasElement, role: u32): { handle: u32; close: () => void } {
  const handle = surface_create(role);
  if (handle === 0) return { handle, close: () => {} };
  bind_surface(handle, canvas);
  const send = (width: u32, height: u32) => surface_size(handle, width, height, dpr());
  const send_box = () => send(round(canvas.clientWidth * dpr()), round(canvas.clientHeight * dpr()));
  const observer = new ResizeObserver((entries) => {
    const entry = entries[entries.length - 1];
    const exact = entry.devicePixelContentBoxSize?.[0];
    if (exact) send(exact.inlineSize, exact.blockSize);
    else send_box();
  });
  try {
    observer.observe(canvas, { box: "device-pixel-content-box" });
  } catch {
    observer.observe(canvas);
  }
  globalThis.addEventListener("resize", send_box);
  return {
    handle,
    close: () => {
      globalThis.removeEventListener("resize", send_box);
      observer.disconnect();
      surface_delete(handle);
      unbind_surface(handle);
    },
  };
}

function pointer_kind(e: PointerEvent): PointerKind {
  return e.pointerType === "touch" ? PointerKind.Touch : e.pointerType === "pen" ? PointerKind.Pen : PointerKind.Mouse;
}

/** Drives the kernel from `canvas`: its box, the pointer over it, the wheel,
 * the tab's visibility. What it returns lets all of it go. */
function drive(canvas: HTMLCanvasElement): () => void {
  const send_rect = () => {
    const r = canvas.getBoundingClientRect();
    kernel_rect(r.left, r.top, r.width, r.height);
  };
  const pointer = (phase: PointerPhase) => (e: PointerEvent) => kernel_pointer(phase, pointer_kind(e), e.pointerId, e.button, e.clientX, e.clientY);
  const on_move = pointer(PointerPhase.Move);
  const on_up = pointer(PointerPhase.Up);
  const on_leave = pointer(PointerPhase.Out);
  const on_down = (e: PointerEvent) => {
    send_rect();
    kernel_pointer(PointerPhase.Down, pointer_kind(e), e.pointerId, e.button, e.clientX, e.clientY);
  };
  const on_wheel = (e: WheelEvent) => {
    if (e.ctrlKey) e.preventDefault();
    kernel_wheel(e.deltaY);
  };
  const on_context_menu = (e: MouseEvent) => e.preventDefault();
  const on_resize = () => {
    send_rect();
    request_frame();
  };
  const on_visibility = () => kernel_visible(document.visibilityState === "visible");
  const on_page_hide = () => kernel_visible(false);

  canvas.addEventListener("pointerdown", on_down);
  canvas.addEventListener("pointermove", on_move);
  canvas.addEventListener("pointerleave", on_leave);
  canvas.addEventListener("contextmenu", on_context_menu);
  canvas.addEventListener("wheel", on_wheel, { passive: false });
  globalThis.addEventListener("resize", on_resize);
  globalThis.addEventListener("scroll", on_resize, true);
  globalThis.addEventListener("pointerup", on_up);
  globalThis.addEventListener("pointercancel", on_up);
  document.addEventListener("visibilitychange", on_visibility);
  globalThis.addEventListener("pagehide", on_page_hide);
  const observer = new ResizeObserver(on_resize);
  observer.observe(canvas);

  send_rect();
  const close = kernel_open(() => ({ width: canvas.clientWidth, height: canvas.clientHeight, dpr: dpr() }));

  return () => {
    close();
    observer.disconnect();
    globalThis.removeEventListener("pagehide", on_page_hide);
    document.removeEventListener("visibilitychange", on_visibility);
    globalThis.removeEventListener("pointercancel", on_up);
    globalThis.removeEventListener("pointerup", on_up);
    globalThis.removeEventListener("scroll", on_resize, true);
    globalThis.removeEventListener("resize", on_resize);
    canvas.removeEventListener("wheel", on_wheel);
    canvas.removeEventListener("contextmenu", on_context_menu);
    canvas.removeEventListener("pointerleave", on_leave);
    canvas.removeEventListener("pointermove", on_move);
    canvas.removeEventListener("pointerdown", on_down);
  };
}

/** The world: the map's surface, and the page driven from it. */
export function WorldSurface() {
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const surface = open_surface(canvas, surface_role.WORLD);
    const stop = drive(canvas);
    return () => {
      stop();
      surface.close();
    };
  }, []);

  return <canvas ref={ref} style={{ ...FILL, touchAction: "none" }} />;
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

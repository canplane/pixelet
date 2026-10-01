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
import { bindSurface, unbindSurface } from "@/platform/web/webgpu.ts";

/** A wheel's line in CSS pixels, for a browser that turns it in lines: the
 * line height a page's default text has. */
const LINE_PX = 16;

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
function openSurface(canvas: HTMLCanvasElement, role: u32): { handle: u32; close: () => void } {
  const handle = surface_create(role);
  if (handle === 0) return { handle, close: () => {} };
  bindSurface(handle, canvas);
  const send = (width: u32, height: u32) => surface_size(handle, width, height, dpr());
  const sendBox = () => send(round(canvas.clientWidth * dpr()), round(canvas.clientHeight * dpr()));
  const observer = new ResizeObserver((entries) => {
    const entry = entries[entries.length - 1];
    const exact = entry.devicePixelContentBoxSize?.[0];
    if (exact) send(exact.inlineSize, exact.blockSize);
    else sendBox();
  });
  try {
    observer.observe(canvas, { box: "device-pixel-content-box" });
  } catch {
    observer.observe(canvas);
  }
  globalThis.addEventListener("resize", sendBox);
  return {
    handle,
    close: () => {
      globalThis.removeEventListener("resize", sendBox);
      observer.disconnect();
      surface_delete(handle);
      unbindSurface(handle);
    },
  };
}

function pointerKind(e: PointerEvent): PointerKind {
  return e.pointerType === "touch" ? PointerKind.Touch : e.pointerType === "pen" ? PointerKind.Pen : PointerKind.Mouse;
}

/** Drives the kernel from `canvas`: its box, the pointer over it, the wheel,
 * the tab's visibility. What it returns lets all of it go. */
function drive(canvas: HTMLCanvasElement): () => void {
  const sendRect = () => {
    const r = canvas.getBoundingClientRect();
    kernel_rect(r.left, r.top, r.width, r.height);
  };
  const pointer = (phase: PointerPhase) => (e: PointerEvent) => kernel_pointer(phase, pointerKind(e), e.pointerId, e.button, e.clientX, e.clientY);
  const onMove = pointer(PointerPhase.Move);
  const onUp = pointer(PointerPhase.Up);
  const onLeave = pointer(PointerPhase.Out);
  const onDown = (e: PointerEvent) => {
    sendRect();
    kernel_pointer(PointerPhase.Down, pointerKind(e), e.pointerId, e.button, e.clientX, e.clientY);
  };
  const onWheel = (e: WheelEvent) => {
    if (e.ctrlKey) e.preventDefault();
    // A turn may come in lines or pages rather than the pixels the kernel
    // takes it in.
    const per = e.deltaMode === WheelEvent.DOM_DELTA_LINE ? LINE_PX : e.deltaMode === WheelEvent.DOM_DELTA_PAGE ? globalThis.innerHeight : 1;
    kernel_wheel(e.deltaY * per);
  };
  const onContextMenu = (e: MouseEvent) => e.preventDefault();
  const onResize = () => {
    sendRect();
    request_frame();
  };
  const onVisibility = () => kernel_visible(document.visibilityState === "visible");
  const onPageHide = () => kernel_visible(false);

  canvas.addEventListener("pointerdown", onDown);
  canvas.addEventListener("pointermove", onMove);
  canvas.addEventListener("pointerleave", onLeave);
  canvas.addEventListener("contextmenu", onContextMenu);
  canvas.addEventListener("wheel", onWheel, { passive: false });
  globalThis.addEventListener("resize", onResize);
  globalThis.addEventListener("scroll", onResize, true);
  globalThis.addEventListener("pointerup", onUp);
  globalThis.addEventListener("pointercancel", onUp);
  document.addEventListener("visibilitychange", onVisibility);
  globalThis.addEventListener("pagehide", onPageHide);
  const observer = new ResizeObserver(onResize);
  observer.observe(canvas);

  sendRect();
  const close = kernel_open(() => ({ width: canvas.clientWidth, height: canvas.clientHeight, dpr: dpr() }));

  return () => {
    close();
    observer.disconnect();
    globalThis.removeEventListener("pagehide", onPageHide);
    document.removeEventListener("visibilitychange", onVisibility);
    globalThis.removeEventListener("pointercancel", onUp);
    globalThis.removeEventListener("pointerup", onUp);
    globalThis.removeEventListener("scroll", onResize, true);
    globalThis.removeEventListener("resize", onResize);
    canvas.removeEventListener("wheel", onWheel);
    canvas.removeEventListener("contextmenu", onContextMenu);
    canvas.removeEventListener("pointerleave", onLeave);
    canvas.removeEventListener("pointermove", onMove);
    canvas.removeEventListener("pointerdown", onDown);
  };
}

/** The world: the map's surface, and the page driven from it. */
export function WorldSurface() {
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const surface = openSurface(canvas, surface_role.WORLD);
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
    const surface = openSurface(canvas, surface_role.CASE);
    const close = surface.handle !== 0 ? open.current(surface.handle) : () => {};
    return () => {
      close();
      surface.close();
    };
  }, [openKey]);

  return <canvas ref={ref} style={FILL} />;
}

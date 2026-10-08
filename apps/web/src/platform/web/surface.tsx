// @pxl/web/platform/web/surface.tsx

/**
 * The canvas the kernel draws into on the web, and the page's input driver:
 * its box, pointer, wheel, keys and the tab's visibility.
 */

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";
import { useEffect, useRef } from "react";

import { PointerKind, PointerPhase } from "@/kernel/abi/cmd.spec.ts";
import { kernel_open, kernel_rect, kernel_visible, kernel_wheel } from "@/kernel/control/input.ts";
import { worldPointer } from "@/app/store/pointer.ts";
import { keyDown, keyUp, releaseKeys } from "@/app/store/keyboard.ts";
import { surface_role } from "@/kernel/render/gpu/api.spec.ts";
import { surface_close, surface_open, surface_size } from "@/kernel/render/surface.ts";
import { request_frame } from "@/kernel/sched.ts";
import { bindSurface, unbindSurface } from "@/platform/web/webgpu.ts";

/** A wheel line in CSS pixels: a page's default line height. */
const LINE_PX = 16;

const FILL = { position: "absolute", inset: 0, width: "100%", height: "100%" } as const;

function dpr(): f64 {
  return globalThis.devicePixelRatio || 1;
}

/** Calls `then` when the device pixel ratio changes: a page zoom, or a move to another screen. */
function onRatioChange(then: () => void): () => void {
  let query = matchMedia(`(resolution: ${dpr()}dppx)`);
  const fire = () => {
    query.removeEventListener("change", fire);
    query = matchMedia(`(resolution: ${dpr()}dppx)`);
    query.addEventListener("change", fire);
    then();
  };
  query.addEventListener("change", fire);
  return () => query.removeEventListener("change", fire);
}

/**
 * Follows the layout box, not the bounding rect, which includes transforms a
 * sliding drawer passes through. Safari reports no device pixels, so the CSS
 * box times the ratio stands in, and a change of ratio alone is watched for.
 */
function openSurface(canvas: HTMLCanvasElement, role: u32): { handle: u32; close: () => void } {
  const handle = surface_open(role);
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
  let stopRatio = () => {};
  try {
    observer.observe(canvas, { box: "device-pixel-content-box" });
  } catch {
    observer.observe(canvas);
    stopRatio = onRatioChange(sendBox);
  }
  return {
    handle,
    close: () => {
      stopRatio();
      observer.disconnect();
      surface_close(handle);
      unbindSurface(handle);
    },
  };
}

/** A field's own keys, which the shell routes apart. */
function inField(e: KeyboardEvent): boolean {
  const target = e.target;
  return target instanceof HTMLElement && (target.isContentEditable || target.closest("input, textarea, select") !== null);
}

function pointerKind(e: PointerEvent): PointerKind {
  return e.pointerType === "touch" ? PointerKind.Touch : e.pointerType === "pen" ? PointerKind.Pen : PointerKind.Mouse;
}

function drive(canvas: HTMLCanvasElement): () => void {
  const sendRect = () => {
    const r = canvas.getBoundingClientRect();
    kernel_rect(r.left, r.top, r.width, r.height);
  };
  const pointer = (phase: PointerPhase) => (e: PointerEvent) => worldPointer(phase, pointerKind(e), e.pointerId, e.button, e.clientX, e.clientY);
  const onMove = pointer(PointerPhase.Move);
  const onUp = pointer(PointerPhase.Up);
  const onCancel = pointer(PointerPhase.Cancel);
  const onLeave = pointer(PointerPhase.Out);
  const down = pointer(PointerPhase.Down);
  const onDown = (e: PointerEvent) => {
    sendRect();
    down(e);
  };
  const onWheel = (e: WheelEvent) => {
    if (e.ctrlKey) e.preventDefault();
    const per = e.deltaMode === WheelEvent.DOM_DELTA_LINE ? LINE_PX : e.deltaMode === WheelEvent.DOM_DELTA_PAGE ? globalThis.innerHeight : 1;
    kernel_wheel(e.deltaY * per);
  };
  const onContextMenu = (e: MouseEvent) => e.preventDefault();
  const onResize = () => {
    sendRect();
    request_frame();
  };
  // A composed or shortcut key is the IME's or the browser's; Tab switches the dock mid-word too.
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.repeat || (e.isComposing && e.code !== "Tab") || e.metaKey || e.ctrlKey || e.altKey) return;
    if (keyDown(e.code, { inField: inField(e) })) e.preventDefault();
  };
  const onKeyUp = (e: KeyboardEvent) => keyUp(e.code);
  const onVisibility = () => {
    if (document.visibilityState !== "visible") releaseKeys();
    kernel_visible(document.visibilityState === "visible");
  };
  const onPageHide = () => kernel_visible(false);

  canvas.addEventListener("pointerdown", onDown);
  canvas.addEventListener("pointermove", onMove);
  canvas.addEventListener("pointerleave", onLeave);
  canvas.addEventListener("contextmenu", onContextMenu);
  canvas.addEventListener("wheel", onWheel, { passive: false });
  globalThis.addEventListener("scroll", onResize, true);
  globalThis.addEventListener("pointerup", onUp);
  globalThis.addEventListener("pointercancel", onCancel);
  document.addEventListener("visibilitychange", onVisibility);
  globalThis.addEventListener("pagehide", onPageHide);
  // Captured: react-native-web's TextInput stops a key's propagation before it bubbles here.
  globalThis.addEventListener("keydown", onKeyDown, true);
  globalThis.addEventListener("keyup", onKeyUp, true);
  globalThis.addEventListener("blur", releaseKeys);
  const observer = new ResizeObserver(onResize);
  observer.observe(canvas);

  sendRect();
  const close = kernel_open(() => ({ width: canvas.clientWidth, height: canvas.clientHeight, dpr: dpr() }));

  return () => {
    close();
    observer.disconnect();
    globalThis.removeEventListener("blur", releaseKeys);
    globalThis.removeEventListener("keyup", onKeyUp, true);
    globalThis.removeEventListener("keydown", onKeyDown, true);
    globalThis.removeEventListener("pagehide", onPageHide);
    document.removeEventListener("visibilitychange", onVisibility);
    globalThis.removeEventListener("pointercancel", onCancel);
    globalThis.removeEventListener("pointerup", onUp);
    globalThis.removeEventListener("scroll", onResize, true);
    canvas.removeEventListener("wheel", onWheel);
    canvas.removeEventListener("contextmenu", onContextMenu);
    canvas.removeEventListener("pointerleave", onLeave);
    canvas.removeEventListener("pointermove", onMove);
    canvas.removeEventListener("pointerdown", onDown);
  };
}

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

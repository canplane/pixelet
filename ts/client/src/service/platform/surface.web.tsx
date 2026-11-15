// @pxl/client/service/platform/surface.web.tsx

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { clamp, round } from "@xpute/kit/math/scalar.ts";
import { give } from "@xpute/runtime-web";
import { useEffect, useRef } from "react";

import type {
  CharacterSurfaceProps,
  InputDriver,
  PointerKind,
  PointerPhase,
  SurfaceDriver,
  WorldSurfaceProps,
} from "./types.ts";

/** How many CSS pixels one line of a wheel scrolls: a page's default line
 * height. */
const LINE_PX = 16;

/** The most one event of a pinch scales by, either way: about what a wheel's
 * notch zooms, as Chrome's PDF viewer holds it. A pinch's events stay under
 * it, and a wheel turned with ctrl held zooms no faster than without. */
const PINCH_MOST = 1.25;

const FILL = {
  position: "absolute",
  inset: 0,
  width: "100%",
  height: "100%",
} as const;

/** The canvas's layout box, in CSS pixels; null while there is none. */
export function surface_box(): { width: f64; height: f64 } | null {
  const canvas = document.querySelector("canvas");
  return canvas && { width: canvas.clientWidth, height: canvas.clientHeight };
}

function dpr(): f64 {
  return globalThis.devicePixelRatio || 1;
}

/** The device pixel ratio changes with a page zoom or a move to another
 * screen. */
function on_ratio_change(then: () => void): () => void {
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
 * The bounding rect is not used for a surface's size, because it includes the
 * transforms that a sliding drawer passes through. Safari reports no size in
 * device pixels, so the CSS box times the pixel ratio stands in, and a change
 * of the ratio alone is watched for.
 */
function open_surface(
  canvas: HTMLCanvasElement,
  surface: SurfaceDriver,
  opened?: (handle: u32) => () => void,
): () => void {
  let close = () => {};
  let closed = false;
  void surface.open().then((handle) => {
    if (handle === 0) return;
    // The surface closed before the core answered, so the handle is let go at
    // once.
    if (closed) return surface.close(handle);
    close = attach(canvas, surface, handle, opened);
  });
  return () => {
    closed = true;
    close();
  };
}

function attach(
  canvas: HTMLCanvasElement,
  surface: SurfaceDriver,
  handle: u32,
  opened?: (handle: u32) => () => void,
): () => void {
  // The canvas is lent to the GPU beside the link, under the surface's
  // handle, until the surface closes.
  give(handle, canvas.transferControlToOffscreen());
  const send = (width: u32, height: u32) =>
    surface.size(handle, width, height, dpr());
  const send_box = () =>
    send(round(canvas.clientWidth * dpr()), round(canvas.clientHeight * dpr()));
  const observer = new ResizeObserver((entries) => {
    const entry = entries[entries.length - 1];
    const exact = entry.devicePixelContentBoxSize?.[0];
    if (exact) send(exact.inlineSize, exact.blockSize);
    else send_box();
  });
  let stop_ratio = () => {};
  try {
    observer.observe(canvas, { box: "device-pixel-content-box" });
  } catch {
    observer.observe(canvas);
    stop_ratio = on_ratio_change(send_box);
  }
  const after = opened?.(handle) ?? (() => {});
  return () => {
    after();
    stop_ratio();
    observer.disconnect();
    surface.close(handle);
  };
}

function in_field(e: KeyboardEvent): boolean {
  const target = e.target;
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable ||
      target.closest("input, textarea, select") !== null)
  );
}

function pointer_kind(e: PointerEvent): PointerKind {
  return e.pointerType === "touch"
    ? "touch"
    : e.pointerType === "pen"
      ? "pen"
      : "mouse";
}

function drive(canvas: HTMLCanvasElement, input: InputDriver): () => void {
  const { key_down, key_up, release_keys, release_pointers } = input;
  // Where the canvas is drawn on the page, with its transforms. A pointer's
  // client position is read against it and scaled into the layout box's CSS
  // pixels.
  let rect = canvas.getBoundingClientRect();
  const place = () => {
    rect = canvas.getBoundingClientRect();
  };
  // A primary press with Shift held is taken as a secondary one, so that a
  // trackpad turns the view with one finger as well as with two pressed.
  const pointer = (phase: PointerPhase) => (e: PointerEvent) =>
    input.pointer(
      phase,
      pointer_kind(e),
      e.pointerId,
      e.button === 0 && e.shiftKey ? 2 : e.button,
      ((e.clientX - rect.left) * canvas.clientWidth) / (rect.width || 1),
      ((e.clientY - rect.top) * canvas.clientHeight) / (rect.height || 1),
    );
  const on_move = pointer("move");
  const on_up = pointer("up");
  const on_cancel = pointer("cancel");
  const on_leave = pointer("out");
  const down = pointer("down");
  const on_down = (e: PointerEvent) => {
    place();
    down(e);
  };
  // A trackpad's pinch comes as a wheel with ctrl held, whose deltaY is -100
  // times the log of the pinch's scale, in Chrome, Safari and Firefox alike.
  // The page's zoom, its default, is prevented. A wheel turned with ctrl
  // held cannot be told from it (`PINCH_MOST`).
  const on_wheel = (e: WheelEvent) => {
    const per =
      e.deltaMode === WheelEvent.DOM_DELTA_LINE
        ? LINE_PX
        : e.deltaMode === WheelEvent.DOM_DELTA_PAGE
          ? globalThis.innerHeight
          : 1;
    if (!e.ctrlKey) return input.wheel(e.deltaY * per);
    e.preventDefault();
    const scale = Math.exp((-e.deltaY * per) / 100);
    input.pinch(clamp(scale, 1 / PINCH_MOST, PINCH_MOST));
  };
  const on_ctx_menu = (e: MouseEvent) => e.preventDefault();
  // A key in a composition belongs to the IME, and a shortcut to the browser,
  // so neither goes to the core. Tab still goes in a composition, because it
  // switches the dock even in the middle of a word.
  const on_key_down = (e: KeyboardEvent) => {
    if (
      e.repeat ||
      (e.isComposing && e.code !== "Tab") ||
      e.metaKey ||
      e.ctrlKey ||
      e.altKey
    )
      return;
    if (key_down(e.code, { in_field: in_field(e) })) e.preventDefault();
  };
  const on_key_up = (e: KeyboardEvent) => key_up(e.code);
  // The core hears of the hiding from the platform (visibility.web.ts), so this
  // only lets go of what the world holds.
  const on_visibility = () => {
    if (document.visibilityState !== "visible") {
      release_keys();
      release_pointers();
    }
  };

  canvas.addEventListener("pointerdown", on_down);
  canvas.addEventListener("pointermove", on_move);
  canvas.addEventListener("pointerleave", on_leave);
  canvas.addEventListener("contextmenu", on_ctx_menu);
  canvas.addEventListener("wheel", on_wheel, { passive: false });
  globalThis.addEventListener("scroll", place, true);
  globalThis.addEventListener("pointerup", on_up);
  globalThis.addEventListener("pointercancel", on_cancel);
  document.addEventListener("visibilitychange", on_visibility);
  // Keys are heard in the capture phase, because react-native-web's TextInput
  // stops a key's propagation before it would bubble up to here.
  globalThis.addEventListener("keydown", on_key_down, true);
  globalThis.addEventListener("keyup", on_key_up, true);
  globalThis.addEventListener("blur", release_keys);
  globalThis.addEventListener("blur", release_pointers);
  const observer = new ResizeObserver(place);
  observer.observe(canvas);

  return () => {
    release_pointers();
    observer.disconnect();
    globalThis.removeEventListener("blur", release_pointers);
    globalThis.removeEventListener("blur", release_keys);
    globalThis.removeEventListener("keyup", on_key_up, true);
    globalThis.removeEventListener("keydown", on_key_down, true);
    document.removeEventListener("visibilitychange", on_visibility);
    globalThis.removeEventListener("pointercancel", on_cancel);
    globalThis.removeEventListener("pointerup", on_up);
    globalThis.removeEventListener("scroll", place, true);
    canvas.removeEventListener("wheel", on_wheel);
    canvas.removeEventListener("contextmenu", on_ctx_menu);
    canvas.removeEventListener("pointerleave", on_leave);
    canvas.removeEventListener("pointermove", on_move);
    canvas.removeEventListener("pointerdown", on_down);
  };
}

export function World({
  surface: driver,
  input,
  overMarker,
}: WorldSurfaceProps) {
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const close = open_surface(canvas, driver);
    const stop = drive(canvas, input);
    return () => {
      stop();
      close();
    };
  }, []);

  return (
    <canvas
      ref={ref}
      style={{
        ...FILL,
        touchAction: "none",
        cursor: overMarker ? "pointer" : undefined,
      }}
    />
  );
}

export function CharacterSurface({
  surface: driver,
  onOpen,
  openKey,
}: CharacterSurfaceProps) {
  const ref = useRef<HTMLCanvasElement>(null);
  const open = useRef(onOpen);
  open.current = onOpen;

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    return open_surface(canvas, driver, (handle) => open.current(handle));
  }, [openKey]);

  // Each surface gets a canvas of its own, because a canvas crosses to the
  // core's worker once and for good.
  return <canvas key={openKey} ref={ref} style={FILL} />;
}

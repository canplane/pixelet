// @pxl/client/service/platform/surface.web.tsx

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { clamp, round } from "@xpute/kit/math/scalar.ts";
import { give } from "@xpute/runtime-web";
import { useEffect, useRef } from "react";

import type {
  CharacterSurfaceProps,
  InputDriver,
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
export function surfaceBox(): { width: f64; height: f64 } | null {
  const canvas = document.querySelector("canvas");
  return canvas && { width: canvas.clientWidth, height: canvas.clientHeight };
}

function dpr(): f64 {
  return globalThis.devicePixelRatio || 1;
}

/** The device pixel ratio changes with a page zoom or a move to another
 * screen. */
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
 * The bounding rect is not used for a surface's size, because it includes the
 * transforms that a sliding drawer passes through. Safari reports no size in
 * device pixels, so the CSS box times the pixel ratio stands in, and a change
 * of the ratio alone is watched for.
 */
function openSurface(
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
  const sendBox = () =>
    send(round(canvas.clientWidth * dpr()), round(canvas.clientHeight * dpr()));
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
  const after = opened?.(handle) ?? (() => {});
  return () => {
    after();
    stopRatio();
    observer.disconnect();
    surface.close(handle);
  };
}

function inField(e: KeyboardEvent): boolean {
  const target = e.target;
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable ||
      target.closest("input, textarea, select") !== null)
  );
}

/** Whether the press under way, or the last, is a primary one with Shift
 * held, which turns the view as a secondary one does, so that a trackpad
 * turns it with one finger as well as with two pressed. */
let shifted = false;

export function pressShifted(): boolean {
  return shifted;
}

function drive(canvas: HTMLCanvasElement, input: InputDriver): () => void {
  const { keyDown, keyUp, releaseKeys } = input;
  // Heard in the capture phase, before the gestures hear the press.
  const onPress = (e: PointerEvent) => {
    shifted = e.button === 0 && e.shiftKey;
  };
  // A trackpad's pinch comes as a wheel with ctrl held, whose deltaY is -100
  // times the log of the pinch's scale, in Chrome, Safari and Firefox alike.
  // The page's zoom, its default, is prevented. A wheel turned with ctrl
  // held cannot be told from it (`PINCH_MOST`).
  const onWheel = (e: WheelEvent) => {
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
  // A key in a composition belongs to the IME, and a shortcut to the browser,
  // so neither goes to the core. Tab still goes in a composition, because it
  // switches the dock even in the middle of a word.
  const onKeyDown = (e: KeyboardEvent) => {
    if (
      e.repeat ||
      (e.isComposing && e.code !== "Tab") ||
      e.metaKey ||
      e.ctrlKey ||
      e.altKey
    )
      return;
    if (keyDown(e.code, { inField: inField(e) })) e.preventDefault();
  };
  const onKeyUp = (e: KeyboardEvent) => keyUp(e.code);
  // The core hears of the hiding from the platform (visibility.web.ts), so this
  // only lets go of the keys the world holds.
  const onVisibility = () => {
    if (document.visibilityState !== "visible") releaseKeys();
  };

  globalThis.addEventListener("pointerdown", onPress, true);
  canvas.addEventListener("wheel", onWheel, { passive: false });
  document.addEventListener("visibilitychange", onVisibility);
  // Keys are heard in the capture phase, because react-native-web's TextInput
  // stops a key's propagation before it would bubble up to here.
  globalThis.addEventListener("keydown", onKeyDown, true);
  globalThis.addEventListener("keyup", onKeyUp, true);
  globalThis.addEventListener("blur", releaseKeys);

  return () => {
    globalThis.removeEventListener("blur", releaseKeys);
    globalThis.removeEventListener("keyup", onKeyUp, true);
    globalThis.removeEventListener("keydown", onKeyDown, true);
    document.removeEventListener("visibilitychange", onVisibility);
    canvas.removeEventListener("wheel", onWheel);
    globalThis.removeEventListener("pointerdown", onPress, true);
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
    const close = openSurface(canvas, driver);
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
    return openSurface(canvas, driver, (handle) => open.current(handle));
  }, [openKey]);

  // Each surface gets a canvas of its own, because a canvas crosses to the
  // core's worker once and for good.
  return <canvas key={openKey} ref={ref} style={FILL} />;
}

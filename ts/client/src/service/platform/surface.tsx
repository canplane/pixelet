// @pxl/client/service/platform/surface.tsx

/**
 * The canvases the core draws into on a page, and the page's input: the
 * canvas's box, the pointer, the wheel, the keys and the tab's visibility.
 */

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";
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

const FILL = {
  position: "absolute",
  inset: 0,
  width: "100%",
  height: "100%",
} as const;

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
 * Opens a surface on `canvas` and keeps it sized to the canvas's layout box.
 * The bounding rect is not used, because it includes the transforms that a
 * sliding drawer passes through. Safari reports no size in device pixels, so
 * the CSS box times the pixel ratio stands in, and a change of the ratio alone
 * is watched for.
 */
function openSurface(
  canvas: HTMLCanvasElement,
  surface: SurfaceDriver,
  opened?: (handle: u32) => () => void,
): () => void {
  let close = () => {};
  let gone = false;
  void surface.open().then((handle) => {
    if (handle === 0) return;
    // The surface closed before the core answered, so the handle is let go at
    // once.
    if (gone) return surface.close(handle);
    close = attach(canvas, surface, handle, opened);
  });
  return () => {
    gone = true;
    close();
  };
}

/** Binds `handle` to the canvas and keeps it sized to it. The returned
 * function lets the handle go. */
function attach(
  canvas: HTMLCanvasElement,
  surface: SurfaceDriver,
  handle: u32,
  opened?: (handle: u32) => () => void,
): () => void {
  surface.bind(handle, canvas);
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
    surface.unbind(handle);
  };
}

/** Whether a key went to a text field. The UI routes a field's keys apart. */
function inField(e: KeyboardEvent): boolean {
  const target = e.target;
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable ||
      target.closest("input, textarea, select") !== null)
  );
}

function pointerKind(e: PointerEvent): PointerKind {
  return e.pointerType === "touch"
    ? "touch"
    : e.pointerType === "pen"
      ? "pen"
      : "mouse";
}

function drive(canvas: HTMLCanvasElement, input: InputDriver): () => void {
  const { keyDown, keyUp, releaseKeys, releasePointers } = input;
  // Where the canvas is drawn on the page, with its transforms. A pointer's
  // client position is read against it and scaled into the layout box's CSS
  // pixels.
  let rect = canvas.getBoundingClientRect();
  const place = () => {
    rect = canvas.getBoundingClientRect();
  };
  const pointer = (phase: PointerPhase) => (e: PointerEvent) =>
    input.pointer(
      phase,
      pointerKind(e),
      e.pointerId,
      e.button,
      ((e.clientX - rect.left) * canvas.clientWidth) / (rect.width || 1),
      ((e.clientY - rect.top) * canvas.clientHeight) / (rect.height || 1),
    );
  const onMove = pointer("move");
  const onUp = pointer("up");
  const onCancel = pointer("cancel");
  const onLeave = pointer("out");
  const down = pointer("down");
  const onDown = (e: PointerEvent) => {
    place();
    down(e);
  };
  const onWheel = (e: WheelEvent) => {
    if (e.ctrlKey) e.preventDefault();
    const per =
      e.deltaMode === WheelEvent.DOM_DELTA_LINE
        ? LINE_PX
        : e.deltaMode === WheelEvent.DOM_DELTA_PAGE
          ? globalThis.innerHeight
          : 1;
    input.wheel(e.deltaY * per);
  };
  const onContextMenu = (e: MouseEvent) => e.preventDefault();
  const onResize = () => {
    place();
    input.requestFrame();
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
  // The keys and pointers the world holds are released as the page hides. The
  // core hears of the hiding itself from the platform (visibility.ts).
  const onVisibility = () => {
    if (document.visibilityState !== "visible") {
      releaseKeys();
      releasePointers();
    }
  };

  canvas.addEventListener("pointerdown", onDown);
  canvas.addEventListener("pointermove", onMove);
  canvas.addEventListener("pointerleave", onLeave);
  canvas.addEventListener("contextmenu", onContextMenu);
  canvas.addEventListener("wheel", onWheel, { passive: false });
  globalThis.addEventListener("scroll", onResize, true);
  globalThis.addEventListener("pointerup", onUp);
  globalThis.addEventListener("pointercancel", onCancel);
  document.addEventListener("visibilitychange", onVisibility);
  // Keys are heard in the capture phase, because react-native-web's TextInput
  // stops a key's propagation before it would bubble up to here.
  globalThis.addEventListener("keydown", onKeyDown, true);
  globalThis.addEventListener("keyup", onKeyUp, true);
  globalThis.addEventListener("blur", releaseKeys);
  globalThis.addEventListener("blur", releasePointers);
  const observer = new ResizeObserver(onResize);
  observer.observe(canvas);

  const close = input.open();

  return () => {
    releasePointers();
    close();
    observer.disconnect();
    globalThis.removeEventListener("blur", releasePointers);
    globalThis.removeEventListener("blur", releaseKeys);
    globalThis.removeEventListener("keyup", onKeyUp, true);
    globalThis.removeEventListener("keydown", onKeyDown, true);
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

  // Over a marker that a tap acts on, the cursor shows as it does over a link.
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

// @pxl/client/service/platform/viewport.ts

import { useEffect } from "react";
import { create } from "zustand";

import type { f64 } from "@xpute/kit/abi/word.ts";
import { VISUAL_VIEWPORT_BOTTOM } from "./types.ts";

const SETTLE_MS = 250;

interface WindowSize {
  width: f64;
  height: f64;
}

function measure(): WindowSize {
  const root = document.documentElement;
  return { width: root.clientWidth, height: root.clientHeight };
}

/** The layout viewport's size, and how far the visual viewport is scrolled
 * down it. */
const store = create<{ size: WindowSize; top: number }>()(() => ({
  size: measure(),
  top: 0,
}));

// The size is observed after layout. A page zoom's `resize` event comes
// before the root has its new size, so a sheet would stay centered on the old
// width.
new ResizeObserver(() => {
  const next = measure();
  const { size } = store.getState();
  if (next.width !== size.width || next.height !== size.height) {
    store.setState({ size: next });
  }
}).observe(document.documentElement);

/**
 * The layout viewport's size, which a keyboard or a pinch leaves alone.
 * React Native for Web's `useWindowDimensions` gives the visual viewport, so a
 * sheet sized from it would lose the keyboard's height at its top.
 */
export function useWindowSize(): WindowSize {
  return store((s) => s.size);
}

export function useViewportTop(): number {
  return store((s) => s.top);
}

function set_top(top: number): void {
  store.setState({ top });
}

interface VirtualKeyboard extends EventTarget {
  overlaysContent: boolean;
  readonly boundingRect: DOMRect;
}

const keyboard = (
  navigator as Navigator & { virtualKeyboard?: VirtualKeyboard }
).virtualKeyboard;

/**
 * Pins the overlays to the visual viewport: the top through `useViewportTop`,
 * and the bottom through `VISUAL_VIEWPORT_BOTTOM` (types.ts). iOS Safari
 * ignores `interactive-widget` and pans the visual viewport instead. Where the
 * VirtualKeyboard API exists, the keyboard overlays the page and reports its
 * own height.
 */
export function useVisualViewport(): void {
  useEffect(() => {
    const vv = globalThis.visualViewport;
    if (!vv) return;
    if (keyboard) keyboard.overlaysContent = true;

    const root = document.documentElement;
    let frame = 0;
    let settleTimer: ReturnType<typeof setTimeout> | 0 = 0;

    function apply() {
      frame = 0;
      if (!vv) return;

      // The bottom is set on every frame, as a CSS variable, so that following
      // it re-renders nothing.
      const bottom = keyboard
        ? keyboard.boundingRect.height
        : Math.max(0, root.clientHeight - (vv.offsetTop + vv.height));
      root.style.setProperty(VISUAL_VIEWPORT_BOTTOM, `${bottom}px`);

      // The top waits until the viewport settles. iOS overshoots while the
      // keyboard animates, and following it live would make the top overlays
      // bounce.
      if (settleTimer !== 0) clearTimeout(settleTimer);
      settleTimer = setTimeout(() => {
        settleTimer = 0;
        if (!vv) return;
        set_top(vv.offsetTop);
      }, SETTLE_MS);
    }

    function schedule() {
      if (frame !== 0) return;
      frame = requestAnimationFrame(apply);
    }

    set_top(vv.offsetTop);
    apply();
    vv.addEventListener("resize", schedule);
    vv.addEventListener("scroll", schedule);
    keyboard?.addEventListener("geometrychange", schedule);

    return () => {
      if (frame !== 0) cancelAnimationFrame(frame);
      if (settleTimer !== 0) clearTimeout(settleTimer);
      vv.removeEventListener("resize", schedule);
      vv.removeEventListener("scroll", schedule);
      keyboard?.removeEventListener("geometrychange", schedule);
      if (keyboard) keyboard.overlaysContent = false;
      root.style.removeProperty(VISUAL_VIEWPORT_BOTTOM);
      set_top(0);
    };
  }, []);
}

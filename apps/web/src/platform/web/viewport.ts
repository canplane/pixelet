// @pxl/web/platform/web/viewport.ts

import { useEffect } from "react";
import { create } from "zustand";

import type { f64 } from "@xpute/core/abi/word.ts";

const SETTLE_MS = 250;

interface WindowSize {
  width: f64;
  height: f64;
}

function measure(): WindowSize {
  const root = document.documentElement;
  return { width: root.clientWidth, height: root.clientHeight };
}

/** The layout viewport's size, and how far the visual viewport is scrolled down it. */
const useViewport = create(() => ({ size: measure(), top: 0 }));

// Observed after layout: a page zoom's `resize` came before the root's new
// size, and a sheet stayed centered on the old width.
new ResizeObserver(() => {
  const next = measure();
  const { size } = useViewport.getState();
  if (next.width !== size.width || next.height !== size.height) useViewport.setState({ size: next });
}).observe(document.documentElement);

/**
 * The layout viewport, which a keyboard or pinch leaves alone. RN-web's
 * `useWindowDimensions` is the visual viewport, so a sheet sized from it lost
 * the keyboard's height at its top.
 */
export function useWindowSize(): WindowSize {
  return useViewport((s) => s.size);
}

export function useViewportTop(): number {
  return useViewport((s) => s.top);
}

function setTop(top: number): void {
  useViewport.setState({ top });
}

interface VirtualKeyboard extends EventTarget {
  overlaysContent: boolean;
  readonly boundingRect: DOMRect;
}

const keyboard = (navigator as Navigator & { virtualKeyboard?: VirtualKeyboard }).virtualKeyboard;

/**
 * Pins the overlays to the visual viewport: the top as `useViewportTop`, the
 * bottom as `--vv-bottom`. iOS Safari ignores `interactive-widget` and pans
 * the visual viewport instead; where the VirtualKeyboard API exists, the
 * keyboard overlays the page and reports its own height.
 */
export function useVisualViewport(): void {
  useEffect(() => {
    const vv = globalThis.visualViewport;
    if (!vv) return;
    if (keyboard) keyboard.overlaysContent = true;

    const root = document.documentElement;
    let frame = 0;
    let settleTimer = 0;

    function apply() {
      frame = 0;
      if (!vv) return;

      // Every frame, as a CSS variable so following it re-renders nothing.
      const bottom = keyboard ? keyboard.boundingRect.height : Math.max(0, root.clientHeight - (vv.offsetTop + vv.height));
      root.style.setProperty("--vv-bottom", `${bottom}px`);

      // Top waits until the viewport settles: iOS overshoots while the keyboard
      // animates, and tracking it live made the top overlays bounce.
      if (settleTimer !== 0) clearTimeout(settleTimer);
      settleTimer = setTimeout(() => {
        settleTimer = 0;
        if (!vv) return;
        setTop(vv.offsetTop);
      }, SETTLE_MS);
    }

    function schedule() {
      if (frame !== 0) return;
      frame = requestAnimationFrame(apply);
    }

    setTop(vv.offsetTop);
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
      root.style.removeProperty("--vv-bottom");
      setTop(0);
    };
  }, []);
}

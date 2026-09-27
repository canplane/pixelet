// @pxl/web/platform/web/viewport.ts

import { useEffect, useSyncExternalStore } from "react";

import type { f64 } from "@xpute/core/abi/word.ts";

const SETTLE_MS = 250;

export interface WindowSize {
  width: f64;
  height: f64;
}

let size: WindowSize = measure();

function measure(): WindowSize {
  const root = document.documentElement;
  return { width: root.clientWidth, height: root.clientHeight };
}

/**
 * The window the chrome is laid out in: the layout viewport, which a keyboard
 * or a pinch leaves where it is. React Native's `useWindowDimensions` is the
 * visual viewport on a page (react-native-web's Dimensions), which shrinks
 * under a keyboard and with a zoom, so a sheet sized from it lost the
 * keyboard's height at its top while its foot stayed under the keys, and
 * moved again as Safari panned to the field. What a keyboard covers is
 * followed by the one mechanism below, and nothing else sizes itself by it.
 */
export function useWindowSize(): WindowSize {
  return useSyncExternalStore(
    (l) => {
      const resize = () => {
        const next = measure();
        if (next.width === size.width && next.height === size.height) return;
        size = next;
        l();
      };
      globalThis.addEventListener("resize", resize);
      return () => globalThis.removeEventListener("resize", resize);
    },
    () => size,
  );
}

/** The visual viewport's settled top offset, and who is reading it. */
let top = 0;
const listeners = new Set<() => void>();

function setTop(value: number): void {
  if (value === top) return;
  top = value;
  for (const l of listeners) l();
}

/** How far the visible top has moved under the layout viewport's, in CSS
 * pixels: what the top overlays stand below. */
export function useViewportTop(): number {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => top,
  );
}

/** Chromium's VirtualKeyboard, which TypeScript's DOM library does not have. */
interface VirtualKeyboard extends EventTarget {
  overlaysContent: boolean;
  readonly boundingRect: DOMRect;
}

const keyboard = (navigator as Navigator & { virtualKeyboard?: VirtualKeyboard }).virtualKeyboard;

/**
 * Follows the visual viewport so the overlay layer can stay pinned to what is
 * actually on screen: the top as `useViewportTop`, the bottom as the
 * `--vv-bottom` CSS variable.
 *
 * `interactive-widget=resizes-content` in the viewport meta is meant to make
 * this unnecessary — the browser shrinks the layout viewport when the
 * keyboard opens, so `absolute top-4` keeps meaning "below the visible top".
 * Chromium honors it; iOS Safari does not, and instead scrolls the visual
 * viewport over an unchanged layout viewport, riding everything positioned
 * against the layout viewport off the top of the screen. The meta tag stays
 * — it's still what makes `env(safe-area-inset-*)` report real values — this
 * just covers the engine that ignores its resize behavior.
 *
 * Where the browser has the VirtualKeyboard API (Chromium, Android's Chrome
 * among them), the keyboard is laid over the page instead: the visual
 * viewport neither shrinks nor pans for it, so nothing at the top moves, and
 * the bottom is the keyboard's own height, which the API reports once as
 * the keys start to rise. Safari has no such switch; there the page follows
 * its pan as above, and an iPhone that wants none of it has the app.
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

      // Bottom tracks every frame. It is what the keyboard actually covers —
      // the message input lives down there — so it has to stay glued to the
      // moving edge rather than arrive after it. A CSS variable rather than
      // a value a component reads, so following it every frame re-renders
      // nothing.
      const bottom = keyboard ? keyboard.boundingRect.height : Math.max(0, root.clientHeight - (vv.offsetTop + vv.height));
      root.style.setProperty("--vv-bottom", `${bottom}px`);

      // Top waits for the viewport to stop moving. iOS pans the visual
      // viewport while the keyboard animates, overshooting before it settles,
      // and tracking that live made the top overlays travel down and back up
      // on their own — visible, and pointless, since nothing up there is ever
      // covered by a keyboard. Being briefly uncorrected costs nothing;
      // animating in sympathy with the system looked wrong. Settled, it moves
      // rarely enough to be a value the overlays re-render on.
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

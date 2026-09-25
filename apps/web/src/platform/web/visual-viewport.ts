// @pxl/web/platform/web/visual-viewport.ts

import { useEffect, useSyncExternalStore } from "react";

const SETTLE_MS = 250;

/** The visual viewport's settled top offset, and who is reading it. */
let top = 0;
const listeners = new Set<() => void>();

function set_top(value: number): void {
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
 */
export function useVisualViewport(): void {
  useEffect(() => {
    const vv = globalThis.visualViewport;
    if (!vv) return;

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
      const bottom = Math.max(0, root.clientHeight - (vv.offsetTop + vv.height));
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

    return () => {
      if (frame !== 0) cancelAnimationFrame(frame);
      if (settleTimer !== 0) clearTimeout(settleTimer);
      vv.removeEventListener("resize", schedule);
      vv.removeEventListener("scroll", schedule);
      root.style.removeProperty("--vv-bottom");
      set_top(0);
    };
  }, []);
}

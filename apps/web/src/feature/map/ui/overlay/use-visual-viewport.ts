// @pxl/web/feature/map/ui/overlay/use-visual-viewport.ts

import { useEffect } from "react";

const SETTLE_MS = 250;

/**
 * Publishes the visual viewport's offset as CSS variables so the overlay
 * layer can stay pinned to what is actually on screen.
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
      // moving edge rather than arrive after it.
      const bottom = Math.max(0, root.clientHeight - (vv.offsetTop + vv.height));
      root.style.setProperty("--vv-bottom", `${bottom}px`);

      // Top waits for the viewport to stop moving. iOS pans the visual
      // viewport while the keyboard animates, overshooting before it settles,
      // and tracking that live made the top overlays travel down and back up
      // on their own — visible, and pointless, since nothing up there is ever
      // covered by a keyboard. Being briefly uncorrected costs nothing;
      // animating in sympathy with the system looked wrong.
      if (settleTimer !== 0) clearTimeout(settleTimer);
      settleTimer = setTimeout(() => {
        settleTimer = 0;
        if (!vv) return;
        root.style.setProperty("--vv-top", `${vv.offsetTop}px`);
      }, SETTLE_MS);
    }

    function schedule() {
      if (frame !== 0) return;
      frame = requestAnimationFrame(apply);
    }

    root.style.setProperty("--vv-top", `${vv.offsetTop}px`);
    apply();
    vv.addEventListener("resize", schedule);
    vv.addEventListener("scroll", schedule);

    return () => {
      if (frame !== 0) cancelAnimationFrame(frame);
      if (settleTimer !== 0) clearTimeout(settleTimer);
      vv.removeEventListener("resize", schedule);
      vv.removeEventListener("scroll", schedule);
      root.style.removeProperty("--vv-top");
      root.style.removeProperty("--vv-bottom");
    };
  }, []);
}

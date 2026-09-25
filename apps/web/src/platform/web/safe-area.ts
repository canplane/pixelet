// @pxl/web/platform/web/safe-area.ts

import { useEffect, useState } from "react";

export interface SafeArea {
  top: number;
  bottom: number;
}

/** What the notch and the home indicator take off the screen's edges, in
 * CSS pixels: the page's `env(safe-area-inset-*)`, which index.html's
 * `viewport-fit=cover` is what makes non-zero. CSS gives the values only to
 * a style, so one is read back off an element that carries them. */
function measure(): SafeArea {
  const probe = document.createElement("div");
  probe.style.cssText = "position:fixed;visibility:hidden;padding-top:env(safe-area-inset-top,0px);padding-bottom:env(safe-area-inset-bottom,0px)";
  document.body.appendChild(probe);
  const style = getComputedStyle(probe);
  const area = { top: parseFloat(style.paddingTop) || 0, bottom: parseFloat(style.paddingBottom) || 0 };
  probe.remove();
  return area;
}

/** The screen's safe area, following a rotation. */
export function useSafeArea(): SafeArea {
  const [area, setArea] = useState(measure);
  useEffect(() => {
    const update = () => setArea(measure());
    globalThis.addEventListener("resize", update);
    return () => globalThis.removeEventListener("resize", update);
  }, []);
  return area;
}

// @pxl/web/platform/web/safe-area.ts

import { useEffect, useState } from "react";

export interface SafeArea {
  top: number;
  bottom: number;
  /** The part of `bottom` that takes a touch; none on a page. */
  tappable: number;
}

/** `env(safe-area-inset-*)` is only given to a style, so it is read back off an element. */
function measure(): SafeArea {
  const probe = document.createElement("div");
  probe.style.cssText = "position:fixed;visibility:hidden;padding-top:env(safe-area-inset-top,0px);padding-bottom:env(safe-area-inset-bottom,0px)";
  document.body.appendChild(probe);
  const style = getComputedStyle(probe);
  const area = { top: parseFloat(style.paddingTop) || 0, bottom: parseFloat(style.paddingBottom) || 0, tappable: 0 };
  probe.remove();
  return area;
}

export function useSafeArea(): SafeArea {
  const [area, setArea] = useState(measure);
  useEffect(() => {
    const update = () => setArea(measure());
    globalThis.addEventListener("resize", update);
    return () => globalThis.removeEventListener("resize", update);
  }, []);
  return area;
}

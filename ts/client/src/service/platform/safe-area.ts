// @pxl/client/service/platform/safe-area.ts

import { useEffect, useState } from "react";

import type { SafeArea } from "./types.ts";

/** `env(safe-area-inset-*)` can only be read through a style, so it is set on
 * a hidden element and read back. */
function measure(): SafeArea {
  const probe = document.createElement("div");
  probe.style.cssText =
    "position:fixed;visibility:hidden;padding-top:env(safe-area-inset-top,0px);padding-bottom:env(safe-area-inset-bottom,0px)";
  document.body.appendChild(probe);
  const style = getComputedStyle(probe);
  const area = {
    top: parseFloat(style.paddingTop) || 0,
    bottom: parseFloat(style.paddingBottom) || 0,
    tappable: 0,
  };
  probe.remove();
  return area;
}

export function useSafeArea(): SafeArea {
  const [area, setArea] = useState(measure);
  // The insets are read after layout, as the window's size is (viewport.ts),
  // and again on each resize, because a rotation moves them.
  useEffect(() => {
    const observer = new ResizeObserver(() => setArea(measure()));
    observer.observe(document.documentElement);
    return () => observer.disconnect();
  }, []);
  return area;
}

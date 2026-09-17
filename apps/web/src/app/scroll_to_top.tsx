// @pxl/web/app/router/helper/scroll_to_top.tsx

import { useEffect } from "react";
import { useLocation, useNavigationType } from "react-router-dom";

/** Reset scroll position on page navigation (Not applied on back) */
export function ScrollToTop() {
  const { pathname } = useLocation();
  const navigationType = useNavigationType(); // "PUSH" | "POP" | "REPLACE"

  useEffect(() => {
    if (navigationType === "POP") return;
    if (typeof window !== "undefined") window.scrollTo(0, 0);
  }, [pathname, navigationType]);

  return null;
}

// @pxl/web/online.ts

import { setOnline } from "@pxl/client/host/online.ts";

/** The browser's own word: a failed fetch alone cannot tell offline from a server that refuses. */
export function onlineWatch(): void {
  const say = () => setOnline(navigator.onLine);
  say();
  addEventListener("online", say);
  addEventListener("offline", say);
}

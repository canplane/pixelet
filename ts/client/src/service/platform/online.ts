// @pxl/client/service/platform/online.ts

/** The browser's own word on whether it is online. A failed fetch alone
 * cannot tell being offline from a server that refuses. */
export function watch(report: (online: boolean) => void): void {
  const say = () => report(navigator.onLine);
  say();
  addEventListener("online", say);
  addEventListener("offline", say);
}

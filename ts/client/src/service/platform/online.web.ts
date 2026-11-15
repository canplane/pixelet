// @pxl/client/service/platform/online.web.ts

/** A failed fetch alone cannot tell being offline from a server that refuses,
 * so the browser's own word is read. */
export function watch(report: (online: boolean) => void): void {
  const say = () => report(navigator.onLine);
  say();
  addEventListener("online", say);
  addEventListener("offline", say);
}

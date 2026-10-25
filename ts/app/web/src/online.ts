// @pxl/web/online.ts

/** The browser's own word: a failed fetch alone cannot tell offline from a server that refuses. */
export function watch(report: (online: boolean) => void): void {
  const say = () => report(navigator.onLine);
  say();
  addEventListener("online", say);
  addEventListener("offline", say);
}

// @pxl/web/visibility.ts

/** The page hidden or shown, as the browser says: a hidden page gets no frame,
 * and `pagehide` is the last sure moment before it may be discarded. */
export function watch(report: (visible: boolean) => void): void {
  document.addEventListener("visibilitychange", () =>
    report(document.visibilityState === "visible"),
  );
  addEventListener("pagehide", () => report(false));
}

// @pxl/client/service/visibility.ts

/** Whether the page is hidden or shown, as the browser says. A hidden page
 * gets no frames, and `pagehide` is the last moment that surely comes before
 * the page may be discarded. */
export function watch(report: (visible: boolean) => void): void {
  document.addEventListener("visibilitychange", () =>
    report(document.visibilityState === "visible"),
  );
  addEventListener("pagehide", () => report(false));
}

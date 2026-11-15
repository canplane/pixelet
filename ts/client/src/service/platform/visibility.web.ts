// @pxl/client/service/platform/visibility.web.ts

/** Whether the page is shown now. */
export function shown(): boolean {
  return document.visibilityState === "visible";
}

/** A hidden page gets no frames, and `pagehide` is the last moment that surely
 * comes before the page may be discarded. The answer stops the watch. */
export function watch(report: (visible: boolean) => void): () => void {
  const changed = () => report(shown());
  const hid = () => report(false);
  document.addEventListener("visibilitychange", changed);
  addEventListener("pagehide", hid);
  return () => {
    document.removeEventListener("visibilitychange", changed);
    removeEventListener("pagehide", hid);
  };
}

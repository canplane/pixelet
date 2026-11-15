// @pxl/client/service/platform/locale.web.ts

export function list(): readonly string[] {
  return navigator.languages ?? [navigator.language];
}

/** Sets the document's language over the static lang="en" that index.html
 * ships for crawlers. */
export function show(lng: string): void {
  document.documentElement.lang = lng;
}

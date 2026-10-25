// @pxl/web/locale.ts

export function list(): readonly string[] {
  return navigator.languages ?? [navigator.language];
}

/** index.html ships a static lang="en" for crawlers. */
export function show(lng: string): void {
  document.documentElement.lang = lng;
}

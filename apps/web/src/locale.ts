// @pxl/web/locale.ts

export function preferredLanguages(): readonly string[] {
  return navigator.languages ?? [navigator.language];
}

/** index.html ships a static lang="en" for crawlers. */
export function showLanguage(lng: string): void {
  document.documentElement.lang = lng;
}

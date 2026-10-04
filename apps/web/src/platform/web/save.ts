// @pxl/web/platform/web/save.ts

export function saveFile(name: string, text: string, mime: string): Promise<void> {
  const url = URL.createObjectURL(new Blob([text], { type: mime }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
  return Promise.resolve();
}

/** Rejects without a user gesture or off a secure origin. */
export function copyText(text: string): Promise<void> {
  return navigator.clipboard.writeText(text);
}

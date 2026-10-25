// @pxl/web/share.ts

export function save(name: string, text: string, mime: string): Promise<void> {
  const url = URL.createObjectURL(new Blob([text], { type: mime }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
  return Promise.resolve();
}

/** Rejects without a user gesture or off a secure origin. */
export function copy(text: string): Promise<void> {
  return navigator.clipboard.writeText(text);
}

// @pxl/web/platform/web/save.ts

/** Hands the user `text` as a file named `name`, of type `mime`: on the web,
 * a download into wherever the browser keeps them. */
export function save_file(name: string, text: string, mime: string): Promise<void> {
  const url = URL.createObjectURL(new Blob([text], { type: mime }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
  return Promise.resolve();
}

/** Puts `text` on the clipboard. A page may refuse it — without a user's
 * gesture, or off a secure origin — and then this rejects. */
export function copy_text(text: string): Promise<void> {
  return navigator.clipboard.writeText(text);
}

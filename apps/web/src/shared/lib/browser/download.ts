// @pxl/web/shared/lib/browser/download.ts

/**
 * Saves the given Blob or Uint8Array data as a local file via the browser's download mechanism.
 */
export function triggerDownload(
  data: Blob | Uint8Array,
  filename: string,
  mimeType: string = "application/octet-stream",
) {
  if (typeof window === "undefined") return;

  let blob: Blob;

  if (data instanceof Blob) {
    // already a Blob (e.g. an image) — its type (image/png etc.) is already set
    blob = data;
  } else {
    // Uint8Array (binary): apply the standard binary type.
    // copy first for SharedArrayBuffer safety — some Uint8Arrays back onto a
    // SharedArrayBuffer, which isn't a valid BlobPart. Constructing a new
    // Uint8Array copies the data into a plain ArrayBuffer instead.
    const safeData = new Uint8Array(data);
    blob = new Blob([safeData], { type: mimeType });
  }

  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;

  // append to the DOM briefly, click, then remove (cross-browser compatibility)
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);

  URL.revokeObjectURL(url);
}

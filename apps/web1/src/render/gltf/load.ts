// @pxl/web/render/gltf/load.ts

/**
 * Shared loading and caching of the models and the skins, so any consumer
 * — the companions, the decor kinds, the picker's preview — hits the
 * network and the parser at most once per URL. Plain promise caches: a
 * consumer that mounts and unmounts as its parent re-renders must not
 * restart a load from scratch.
 */

import { type Gltf, parse_gltf } from "./gltf.ts";

const gltf_cache = new Map<string, Promise<Gltf>>();

/** A parsed glTF by URL. */
export function load_gltf(url: string): Promise<Gltf> {
  let entry = gltf_cache.get(url);
  if (!entry) {
    entry = fetch(url).then(async (res) => {
      if (!res.ok) throw new Error(`gltf: ${url} ${res.status}`);
      return parse_gltf(await res.arrayBuffer());
    });
    gltf_cache.set(url, entry);
  }
  return entry;
}

const image_cache = new Map<string, Promise<ImageBitmap>>();

/** A skin by URL, as the renderer uploads it: the file's own pixels, top
 * row first, no colour conversion and no premultiplied alpha. */
export function load_image(url: string): Promise<ImageBitmap> {
  let entry = image_cache.get(url);
  if (!entry) {
    entry = fetch(url).then(async (res) => {
      if (!res.ok) throw new Error(`image: ${url} ${res.status}`);
      return createImageBitmap(await res.blob(), { colorSpaceConversion: "none", premultiplyAlpha: "none" });
    });
    image_cache.set(url, entry);
  }
  return entry;
}

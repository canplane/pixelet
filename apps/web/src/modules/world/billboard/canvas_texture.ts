// @pxl/web/modules/world/billboard/canvas_texture.ts

import * as THREE from "three";

/**
 * Wraps a canvas into a THREE.CanvasTexture ready for a billboard, with the
 * one filter/colour-space setup every billboard-tier consumer needs.
 *
 * NearestFilter magFilter is what gives billboard content its "pixel" read
 * when the sprite scales up close to the camera — the same texture-filtering
 * choice this codebase's terrain and character textures already make, just
 * applied to a canvas-drawn source instead of a decoded PNG.
 *
 * minFilter stays a mipmapped Linear, deliberately not matching magFilter:
 * distance_fade.ts holds a billboard's on-screen size to a roughly constant
 * CSS-pixel scale, but that is still a slight minification against the
 * canvas's own supersampled resolution (poi_badge.ts and chat_bubble.tsx
 * both have a RENDER_SCALE), and Nearest has no mipmap — every camera
 * micro-movement then resamples a different single texel, which reads as
 * text visibly crawling. Mipmapped Linear filters that away on shrink while
 * magFilter keeps the up-close look untouched; min and mag are independent
 * axes. The canvas has to be drawn supersampled for it to matter, since a
 * mipmap chain only preserves detail and cannot add any.
 *
 * A plain function rather than a hook, because the caller that owns the
 * canvas is the caller that should own the texture. This used to be a
 * `useMemo` inside the component that drew the billboard, so a badge held
 * by id still rebuilt its GPU texture on every mount — the bitmap was
 * cached and the upload was not, which was most of the cost and none of the
 * saving. Whoever builds and keeps the canvas builds and keeps this beside
 * it, and disposes both together.
 */
export function make_canvas_texture(canvas: HTMLCanvasElement): THREE.CanvasTexture {
  const t = new THREE.CanvasTexture(canvas);
  t.magFilter = THREE.NearestFilter;
  t.minFilter = THREE.LinearMipmapLinearFilter;
  t.colorSpace = THREE.SRGBColorSpace;
  return t;
}

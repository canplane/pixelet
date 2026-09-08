// @pxl/web/modules/world/billboard/use_canvas_texture.ts

import { useEffect, useMemo } from "react";
import * as THREE from "three";

/**
 * Wraps a canvas (rebuilt by the caller whenever its own content changes —
 * chat_bubble.tsx rebuilds one per distinct message text) into a
 * THREE.CanvasTexture ready for SpriteBillboard, with the one filter/color-
 * space setup every billboard-tier consumer needs and the disposal a plain
 * useMemo alone can't provide (a CanvasTexture holds a real WebGL resource;
 * without this effect, a newer canvas replacing an older one — or the
 * billboard unmounting — would leak it). NearestFilter magFilter is what
 * gives billboard content its "pixel" read when the sprite scales up close
 * to the camera — same texture-filtering choice this codebase's
 * terrain/character textures already make, just applied to a canvas-drawn
 * source instead of a decoded PNG.
 *
 * minFilter stays a mipmapped Linear, deliberately not matching magFilter:
 * distance_fade.ts holds a billboard's on-screen size to a roughly constant
 * CSS-pixel scale, but that's still a slight minification against the
 * canvas's own supersampled resolution (chat_bubble.tsx's RENDER_SCALE),
 * and Nearest has no mipmap — every camera micro-movement then resamples a
 * different single texel, which reads as text visibly crawling/shimmering.
 * Mipmapped Linear filters that away on shrink while magFilter still keeps
 * the up-close "pixel" look untouched (min/mag are independent axes).
 *
 * `canvas` should still be rendered at a supersampled resolution (see
 * chat_bubble.tsx's own RENDER_SCALE) — the mipmap chain built from it only
 * preserves source detail, it can't add any beyond what's already drawn.
 */
export function use_canvas_texture(canvas: HTMLCanvasElement | null): THREE.CanvasTexture | null {
  const texture = useMemo(() => {
    if (!canvas) return null;
    const t = new THREE.CanvasTexture(canvas);
    t.magFilter = THREE.NearestFilter;
    t.minFilter = THREE.LinearMipmapLinearFilter;
    t.colorSpace = THREE.SRGBColorSpace;
    return t;
  }, [canvas]);

  useEffect(() => {
    if (!texture) return;
    return () => texture.dispose();
  }, [texture]);

  return texture;
}

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
 * billboard unmounting — would leak it). NearestFilter is what actually
 * gives billboard content its "pixel" read: the canvas itself is drawn with
 * regular anti-aliased strokes/text (readability), and Nearest sampling
 * keeps hard edges hard instead of blurring them when the sprite scales up
 * close to the camera — same texture-filtering choice this codebase's
 * terrain/character textures already make, just applied to a canvas-drawn
 * source instead of a decoded PNG.
 *
 * `canvas` should already be rendered at a supersampled resolution (see
 * chat_bubble.tsx's own RENDER_SCALE) if its content includes text or fine
 * detail — NearestFilter alone can only preserve source detail, never add
 * any, so a canvas sized directly off its own logical pixel metrics would
 * still blur once magnified onto a sprite occupying many more screen
 * pixels.
 */
export function use_canvas_texture(canvas: HTMLCanvasElement | null): THREE.CanvasTexture | null {
  const texture = useMemo(() => {
    if (!canvas) return null;
    const t = new THREE.CanvasTexture(canvas);
    t.magFilter = THREE.NearestFilter;
    t.minFilter = THREE.NearestFilter;
    t.colorSpace = THREE.SRGBColorSpace;
    return t;
  }, [canvas]);

  useEffect(() => {
    if (!texture) return;
    return () => texture.dispose();
  }, [texture]);

  return texture;
}

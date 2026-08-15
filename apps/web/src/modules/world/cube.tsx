// @pxl/web/modules/world/cube.tsx
// generic cube primitive, positioned under a parent local/world anchor

import { useEffect, useMemo } from "react";
import * as THREE from "three";

import type { f64 } from "@xpute/core/kit/word.ts";
import { center3, fill3, type V3 } from "@pxl/shared/spec/space/v3.ts";
import { U_PER_CELL } from "@pxl/shared/bake/constants.ts";
import { light_cfg } from "@/modules/world/environment/config.ts";

type CubeProps = {
  base: V3;
  span: V3 | f64;

  color: string;
  /**
   * Optional tiled material (e.g. wool) — repeats once per real U_PER_CELL
   * across the cube's own footprint instead of stretching one image
   * across the whole thing, so an octree-merged multi-cell cube still
   * reads as many unit tiles, not one smeared texture. Done by scaling
   * this cube's own geometry UVs (not the shared texture's own `repeat`,
   * which every differently-sized cube using this color would otherwise
   * fight over — see objects/tex.ts) — which assumes uniform span on
   * every axis, true for every current caller (object cubes, merged or
   * not, are always cubes; only the debug markers in world.tsx pass a
   * scalar span too, so this holds there as well). Falls back to the flat
   * `color` fill when omitted.
   */
  texture?: THREE.Texture;
};
export const Cube = (props: CubeProps) => {
  const { base, span: _span, color, texture } = props;
  const span: V3 = typeof _span === "number" ? fill3(_span) : _span;

  const geometry = useMemo(() => {
    const geo = new THREE.BoxGeometry(span[0], span[1], span[2]);
    if (texture) {
      const repeat: f64 = span[0] / U_PER_CELL;
      const uv = geo.attributes.uv as THREE.BufferAttribute;
      for (let i = 0; i < uv.count; i++) {
        uv.setXY(i, uv.getX(i) * repeat, uv.getY(i) * repeat);
      }
      uv.needsUpdate = true;
    }
    return geo;
  }, [span[0], span[1], span[2], texture]);

  // Imperatively-constructed (not JSX <boxGeometry>), so R3F doesn't own
  // it and won't auto-dispose it on unmount/re-run — that's on us.
  useEffect(() => () => geometry.dispose(), [geometry]);

  return (
    <mesh name="cube" position={center3(base, span)} geometry={geometry}>
      {
        /* key toggles between two distinct component identities (not just
        changed props on one) — three.js compiles MeshStandardMaterial's
        shader once, with a USE_MAP define baked in based on whether `map`
        was truthy AT THAT COMPILE TIME. Flipping `map` from null to a real
        texture afterward on the SAME material instance re-uploads the
        uniform but never re-triggers that compile, so it silently kept
        rendering with the old no-map shader — the flat #ffffff `color`
        this uses as the map's neutral tint, with nothing sampling the
        texture over it, which is exactly "the whole thing goes white" (the
        low_spec toggle's actual symptom) rather than a load-timing issue.
        Keying on texture presence forces React to unmount/remount a fresh
        material instance instead, which compiles correctly from scratch
        every time. */
      }
      <meshStandardMaterial
        key={texture ? "textured" : "flat"}
        color={texture ? "#ffffff" : color}
        map={texture ?? null}
        // light_cfg.FLOOR (world/environment/config.ts) — shared with terrain
        // (modules/world/chunk_stage.tsx), which needs the exact
        // same floor for the exact same reason: both are real lit
        // MeshStandardMaterials driven by environment.tsx's actual scene
        // lights, which genuinely dim hard at night (themes.ts's NIGHT
        // ambient/hemisphere/directional all drop well below daytime), so
        // without a floor either one goes most of the way to black at
        // night. `emissive` uses the cube's own true color (not the
        // texture-mode `color` prop, which is flattened to white so `map`
        // isn't tinted) so the floor still reads as that cube's own hue
        // at night instead of washing every cube toward the same
        // grey/white.
        emissive={color}
        emissiveIntensity={light_cfg.FLOOR}
        roughness={1.0}
        metalness={0.0}
      />
    </mesh>
  );
};

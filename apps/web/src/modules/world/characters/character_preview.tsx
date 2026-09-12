// @pxl/web/modules/world/characters/character_preview.tsx

/**
 * Small standalone R3F canvas for the settings variant picker — renders one
 * character playing its idle clip while slowly spinning in place, so a
 * pick can actually be seen before committing to it. Deliberately separate
 * from companion.tsx's Walker: no map_center, no ground-following, no
 * catch-up walk — this is a display case, not a world entity.
 */

import { useEffect } from "react";
import { Canvas, useFrame, useThree } from "@react-three/fiber";

import type { f64 } from "@xpute/core/kit/word.ts";

import { useCharacterModel } from "./use_character_model.ts";
import type { CharacterVariant } from "@/modules/world/characters/variants.ts";

const SPIN_RATE_RAD_S: f64 = 0.6;
const IDLE_CLIP = "idle";

function PreviewModel(props: { variant: CharacterVariant }) {
  const { variant } = props;
  // Fixed 1-unit height regardless of the variant's real in-world height —
  // this display case only ever compares silhouettes side by side, not
  // relative scale.
  const { root, actions } = useCharacterModel(variant.model_url, variant.skin_url, 1);

  useEffect(() => {
    const action = actions[IDLE_CLIP];
    action?.reset().play();
    return () => void action?.stop();
  }, [actions]);

  useFrame((_state, delta) => {
    if (root) root.rotation.z += SPIN_RATE_RAD_S * delta;
  });

  return root ? <primitive object={root} /> : null;
}

function PreviewCameraSetup() {
  const camera = useThree((state) => state.camera);
  useEffect(() => {
    // use_character_model wraps its glTF (Y-up) so the group's own local Z
    // is "up" (matching this app's Z-up world elsewhere) — this canvas has
    // no such world to inherit that convention from, so the camera itself
    // is set up to treat Z as up instead of fighting the model.
    camera.up.set(0, 0, 1);
    camera.position.set(0, -2.2, 0.9);
    camera.lookAt(0, 0, 0.55);
  }, [camera]);
  return null;
}

export type CharacterPreviewProps = {
  variant: CharacterVariant;
  className?: string;
};

export function CharacterPreview(props: CharacterPreviewProps) {
  const { variant, className } = props;

  return (
    <div className={className}>
      {
        /* frameloop left at its default ("always") — this is one small,
      isolated, permanently-spinning canvas with nothing else to drive
      invalidation, unlike the main scene's frameloop="demand". */
      }
      <Canvas camera={{ fov: 35 }} gl={{ alpha: true }} style={{ background: "transparent" }}>
        <PreviewCameraSetup />
        <PreviewModel variant={variant} />
      </Canvas>
    </div>
  );
}

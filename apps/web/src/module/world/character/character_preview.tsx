// @pxl/web/module/world/character/character_preview.tsx

/**
 * Small display case for the settings variant picker — one character
 * playing its idle clip while slowly spinning in place, so a pick can
 * actually be seen before committing to it. Deliberately separate from
 * companion.tsx's Walker: no map_center, no ground-following, no catch-up
 * walk — a display case, not a world entity.
 *
 * The case is the renderer's (render/gpu/preview.ts): an actor of the
 * module stood far below the world, where nothing looks, and drawn into
 * this canvas alone each frame. The model is registered once more for the
 * case, so the walker's own draws of the same variant are not mistaken for
 * it, at one unit tall: the case compares silhouettes side by side, not
 * relative scale.
 */

import { useEffect, useRef } from "react";

import type { f64, u32 } from "@xpute/core/kit/word.ts";

import { request_frame } from "@/wasm/kernel.ts";
import { load_gltf, load_image } from "@/render/gltf/load.ts";
import { type ModelHandle, register_model } from "@/render/gpu/model_asset.ts";
import { gpu_renderer } from "@/render/gpu/stage.tsx";
import { CORE_PXL } from "@/wasm/core_pool.ts";
import { CORE_BUILD, packet } from "@/wasm/core_build.ts";
import { Cmd } from "@pxl/shared/spec/cmd.ts";
import type { CharacterVariant } from "@/module/world/character/variant.ts";

const SPIN_RATE_RAD_S: f64 = 0.6;
const IDLE_CLIP = "idle";
/** How far below the world origin the case's actor stands: past any
 * terrain, and past the fog, so the world never draws it. */
const CASE_DEPTH_U: f64 = 100_000;

/** The case's own registration of a variant's model, one unit tall. */
const case_models = new Map<string, Promise<ModelHandle | null>>();
function case_model_for(variant: CharacterVariant): Promise<ModelHandle | null> {
  let entry = case_models.get(variant.model_url);
  if (!entry) {
    entry = load_gltf(variant.model_url).then((gltf) => (CORE_PXL ? register_model(CORE_PXL, gltf, 1) : null));
    case_models.set(variant.model_url, entry);
  }
  return entry;
}

export type CharacterPreviewProps = {
  variant: CharacterVariant;
  className?: string;
};

export function CharacterPreview(props: CharacterPreviewProps) {
  const { variant, className } = props;
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = ref.current;
    const gpu = gpu_renderer;
    if (!canvas || !gpu || !CORE_PXL) return;
    const preview = gpu.open_preview(canvas);
    let alive = true;
    let actor: u32 | null = null;
    let spin = 0;
    let last = performance.now();
    let raf = 0;
    const at = [0, 0, -CASE_DEPTH_U];

    Promise.all([case_model_for(variant), load_image(variant.skin_url)]).then(([model, skin]) => {
      if (!alive || !model) return;
      const id = CORE_BUILD.call(Cmd.ACTOR_CREATE, packet((b) => b.u32(model.model)), (actor) => actor);
      if (id < 0) return;
      actor = id;
      preview.model = model.model;
      preview.set_skin(model.model, skin);
      const idle = model.clips.get(IDLE_CLIP);
      if (idle !== undefined) CORE_BUILD.send(id, Cmd.ACTOR_PLAY, packet((b) => b.u32(idle).f32(0)));
    });

    // The case turns on its own clock: the world's tick runs only when
    // asked, and this is what asks while the case is open.
    const frame = (now: number) => {
      if (!alive) return;
      const dt = Math.min(0.1, (now - last) / 1000);
      last = now;
      const rect = canvas.getBoundingClientRect();
      const dpr = Math.min(2, globalThis.devicePixelRatio || 1);
      preview.resize(rect.width * dpr, rect.height * dpr);
      if (actor !== null) {
        spin += SPIN_RATE_RAD_S * dt;
        CORE_BUILD.send(actor, Cmd.ACTOR_UPDATE, packet((b) => b.f64(at[0]).f64(at[1]).f64(at[2]).f64(spin).bool(true)));
      }
      request_frame();
      raf = requestAnimationFrame(frame);
    };
    raf = requestAnimationFrame(frame);

    return () => {
      alive = false;
      cancelAnimationFrame(raf);
      if (actor !== null) CORE_BUILD.send(actor, Cmd.ACTOR_DELETE);
      gpu.close_preview(preview);
    };
  }, [variant]);

  return (
    <div className={className}>
      <canvas ref={ref} className="h-full w-full" />
    </div>
  );
}

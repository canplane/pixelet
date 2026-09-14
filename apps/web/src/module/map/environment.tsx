// @pxl/web/module/map/environment.tsx

/**
 * The environment's inputs, handed to the module
 * (crates/pxl-core/src/environment.rs): the time of day's theme when it
 * changes, and each tick where the map is and what the wall clock says. The
 * module places the sun and the moon, eases the light, the fog and the sky
 * toward the theme, and writes what the renderer draws them with.
 */

import { useEffect } from "react";

import type { f64 } from "@xpute/core/kit/word.ts";
import * as epsg4326 from "@pxl/shared/spec/space/adapter/4326.ts";
import type { G4326 } from "@pxl/shared/spec/space/adapter/4326.ts";
import type { PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { light_cfg, render_cfg } from "@/module/world/environment/config.ts";
import { themes, WorldThemeKey } from "@/module/world/environment/theme.ts";
import { useThemeKey } from "@/module/world/environment/theme_clock.ts";
import { camera_cfg } from "@/module/camera/config.ts";
import { useMapStore } from "@/module/map/store.ts";
import { useTick } from "@/module/world/sched/tick.ts";
import { request_world_tick } from "@/module/world/change.ts";
import { CORE_PXL, FIXED_ALLOC } from "@/render/layer/pipeline/core_pool.ts";
import { linear_rgb } from "@/render/gpu/color.ts";
import { set_sky_radius } from "@/render/gpu/stage.tsx";

const { FOG_NEAR, FOG_FAR, FAR_CLIP } = render_cfg;
const { MAX_DIST } = camera_cfg.view;

/** crates/pxl-core/src/environment.rs's THEME_FLOATS. */
const THEME_FLOATS = 34;

// Same Seoul coordinate use_map_bootstrap.ts seeds map_center with before a
// real GPS fix (or that fallback itself) resolves — reused here so the very
// first frame(s), before map_center is available, still get a real location
// instead of computing sun/moon position for (0,0) (equator/prime meridian).
const FALLBACK_LAT = 37.5665;
const FALLBACK_LON = 126.978;

// The sky is the inside of a sphere centred on the world origin, and must
// stay inside the camera's far clip plane. The camera itself can sit up to
// MAX_DIST away from that origin, and looking toward the far side of the
// sphere from an off-centre point adds that same distance again; leaving out
// the margin clips the sky exactly where a wide-shot camera is most likely
// to be looking.
const SKY_DOME_SCALE = (FAR_CLIP - MAX_DIST) * 0.9;
set_sky_radius(SKY_DOME_SCALE);

// The sun and the moon move with the wall clock, a full sweep every 24 real
// hours, and nothing else asks for a frame while the view is still. A
// transition between themes asks for its own frames through the module's
// tick; the clock only needs one this often.
const ENV_CLOCK_FRAME_MS = 2000;

/** The theme as the module takes it: colours in linear light. */
function write_theme(lane: Float32Array, key: WorldThemeKey): void {
  const t = themes[key];
  const l = t.lighting;
  let k = 0;
  const rgb = (hex: string) => {
    lane.set(linear_rgb(hex), k);
    k += 3;
  };
  const one = (v: f64) => {
    lane[k++] = v;
  };
  rgb(t.fog);
  rgb(t.sky.colors.top);
  rgb(t.sky.colors.middle);
  rgb(t.sky.colors.horizon);
  rgb(l.directional.color);
  rgb(l.ambient.color);
  one(l.ambient.intensity);
  rgb(l.hemisphere.sky);
  rgb(l.hemisphere.ground);
  one(l.hemisphere.intensity);
  rgb(l.directional.color);
  one(l.directional.intensity);
  one(key === WorldThemeKey.NIGHT ? 1 : 0);
  one(FOG_NEAR);
  one(FOG_FAR);
  one(light_cfg.FLOOR);
}

const $latlon: G4326 = [0, 0, 0];
let observed_centre: PVF3 | null = null;
let observed_lat = FALLBACK_LAT;
let observed_lon = FALLBACK_LON;

export function MapEnvironment() {
  const theme_key = useThemeKey();

  useEffect(() => {
    const p = CORE_PXL;
    if (!p) return;
    const lane = FIXED_ALLOC.take(Float32Array, THEME_FLOATS);
    write_theme(lane, theme_key);
    p.pxl_env_theme(lane.byteOffset);
    FIXED_ALLOC.give(lane);
    request_world_tick();
  }, [theme_key]);

  useEffect(() => {
    const timer = setInterval(request_world_tick, ENV_CLOCK_FRAME_MS);
    return () => clearInterval(timer);
  }, []);

  // Before the chunks' step, which runs the module's tick.
  useTick("interaction", () => {
    const p = CORE_PXL;
    if (!p) return;
    const centre = useMapStore.getState().map_center;
    if (centre !== observed_centre) {
      observed_centre = centre;
      if (centre) [observed_lon, observed_lat] = epsg4326.unproject($latlon, centre.i, centre.f);
    }
    p.pxl_env_observe(observed_lat, observed_lon, Date.now());
  }, "environment");

  return null;
}

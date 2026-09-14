// @pxl/web/module/world/marker/location_puck.tsx

import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";

import { BOOL, type f64, type i32, type u32 } from "@xpute/core/kit/word.ts";

import { ivec, projection, type PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { add3, center3n, fill3, floor3n, set3, type V3 } from "@pxl/shared/spec/space/v3.ts";

import { U_PER_CELL } from "@pxl/shared/bake/constant.ts";
import { register_tile_interest, useTileStore } from "../terrain/store.ts";
import { render_cfg } from "../environment/config.ts";
import { get_ele } from "../terrain/elevation.ts";

import { WorldThemeKey } from "../environment/theme.ts";
import { useThemeKey } from "../environment/theme_clock.ts";
import { CORE_PXL, FIXED_ALLOC } from "@/wasm/core_pool.ts";
import { request_world_tick } from "../change.ts";

const { FOG_FAR } = render_cfg;

type ZoneSample = {
  off: V3;
  borders: [f64, f64, f64, f64]; // top, bottom, right, left
};

// Numerator (4) is a world-space distance, not a cell count — it fixes the zone's
// footprint size independent of U_PER_CELL. The .707 shave is a fixed one-cell
// diagonal correction in CELL units, so it does not scale with U_PER_CELL either.
const ZONE_RADIUS_CELL: f64 = 4 / U_PER_CELL - .707;
const ZONE_RADIUS2_CELL: f64 = ZONE_RADIUS_CELL * ZONE_RADIUS_CELL;

const ZONE_SAMPLES: ZoneSample[] = (() => {
  const range: i32 = Math.ceil(ZONE_RADIUS_CELL);

  const out: ZoneSample[] = [];
  for (let x: i32 = -range; x <= range; x++) {
    for (let y: i32 = -range; y <= range; y++) {
      const dx: f64 = x;
      const dy: f64 = y;
      const dx2: f64 = dx * dx;
      const dy2: f64 = dy * dy;

      if (dx2 + dy2 > ZONE_RADIUS2_CELL) continue;

      const d_top: f64 = dx2 + (dy + 1) * (dy + 1);
      const d_bottom: f64 = dx2 + (dy - 1) * (dy - 1);
      const d_right: f64 = (dx + 1) * (dx + 1) + dy2;
      const d_left: f64 = (dx - 1) * (dx - 1) + dy2;

      out.push({
        off: [dx * U_PER_CELL, dy * U_PER_CELL, -U_PER_CELL],
        borders: [
          BOOL(d_top > ZONE_RADIUS2_CELL),
          BOOL(d_bottom > ZONE_RADIUS2_CELL),
          BOOL(d_right > ZONE_RADIUS2_CELL),
          BOOL(d_left > ZONE_RADIUS2_CELL),
        ],
      });
    }
  }
  return out;
})();

type LocationPuckProps = {
  device_coord: PVF3 | null;
};
export function LocationPuck(props: LocationPuckProps) {
  const { device_coord } = props;

  const $v = useRef<V3>([0, 0, 0]);

  const theme_key = useThemeKey();

  // Holds the last known device position across a transient GPS dropout (device_coord
  // going null). Without this, the caller previously remounted this component on every
  // such blip, tearing down and rebuilding the zone — that remount, not
  // the position math, was the source of a "stacked/cube" rendering glitch after tab-away-and-back.
  const last_known_device_coord = useRef<PVF3 | null>(null);
  if (device_coord !== null) last_known_device_coord.current = device_coord;
  const effective_device_coord: PVF3 | null = last_known_device_coord.current;

  const own_page_idx = effective_device_coord !== null ? L3_IDX(effective_device_coord.i) : 0n;
  const tile = useTileStore((state) => state.get_tile(own_page_idx));

  const [recompute_tick, set_recompute_tick] = useState(0);
  // The zone ring samples get_ele() across its own L3 page plus its immediate
  // neighbors, which can reach a page not yet loaded on first mount. Without this,
  // those samples would fall back to a flat guess and never get revisited once the
  // missing page streams in.
  useEffect(() => {
    return register_tile_interest(
      (page_idx) => Number(ivec.max_comp(ivec.abs(ivec.sub(page_idx, own_page_idx)))) <= 1,
      () => set_recompute_tick((t) => t + 1),
    );
  }, [own_page_idx]);
  // null (not yet grounded) gates rendering below instead of rendering the marker at
  // a guessed height while the device's own page isn't loaded yet.
  const last_known_center_z = useRef<f64 | null>(null);
  const center_pos: V3 = useMemo(() => {
    if (effective_device_coord === null) return fill3(0);
    const pos: V3 = projection.unproject(fill3(0), effective_device_coord.i, effective_device_coord.f);
    const ele: f64 | null = get_ele(effective_device_coord.i, effective_device_coord.f);
    if (ele !== null) last_known_center_z.current = ele;
    pos[2] = last_known_center_z.current ?? 0;
    return floor3n(pos, U_PER_CELL);
  }, [effective_device_coord, tile]);

  const has_ground: boolean = effective_device_coord !== null && last_known_center_z.current !== null;

  // center_pos floors to the cell grid to line up with the zone ring's cell-sized
  // boxes; the beacon has no such constraint, so this adds the remainder back —
  // otherwise it would quantize to whole-cell jumps instead of moving smoothly.
  const beacon_offset: [f64, f64] = useMemo(() => {
    if (effective_device_coord === null) return [0, 0];
    const pos: V3 = projection.unproject(fill3(0), effective_device_coord.i, effective_device_coord.f);
    return [pos[0] - center_pos[0], pos[1] - center_pos[1]];
  }, [effective_device_coord, center_pos]);

  const is_night: boolean = theme_key === WorldThemeKey.NIGHT;

  // The puck is the core's (crates/pxl-core/src/world/marker.rs): this decides
  // where the device is, which cells the zone covers at what height and
  // where the beacon should stand; the core holds the zone, eases, spins and
  // bobs the beacon, and draws both.
  useLayoutEffect(() => {
    const p = CORE_PXL;
    if (!p) return;
    if (!has_ground) {
      p.pxl_puck_clear();
      return;
    }
    const v: V3 = $v.current;
    const lane = FIXED_ALLOC.take(Float32Array, ZONE_SAMPLES.length * 7);
    for (let i: u32 = 0; i < ZONE_SAMPLES.length; i++) {
      const { off, borders } = ZONE_SAMPLES[i];
      add3(center_pos, off, v);
      const coord: PVF3 = projection.project({ i: 0n, f: 0n }, ...v);
      // Falls back to flat (center height) rather than 0 when a sample lands on an
      // unloaded neighboring page — absolute 0 would sink/launch that instance away
      // from the rest of the zone.
      const delta_ele: f64 = (get_ele(coord.i, coord.f) ?? center_pos[2]) - center_pos[2];
      set3(off[0], off[1], off[2] + delta_ele, v);
      center3n(v, U_PER_CELL, v);
      lane.set([v[0], v[1], v[2], borders[0], borders[1], borders[2], borders[3]], i * 7);
    }
    p.pxl_puck_zone(center_pos[0], center_pos[1], center_pos[2], ZONE_SAMPLES.length, lane.byteOffset, ZONE_RADIUS_CELL * U_PER_CELL, FOG_FAR);
    FIXED_ALLOC.give(lane);
    request_world_tick();
  }, [has_ground, center_pos, recompute_tick]);
  useEffect(() => {
    CORE_PXL?.pxl_puck_beacon(beacon_offset[0], beacon_offset[1], is_night ? 1 : 0);
  }, [beacon_offset, is_night, center_pos]);
  useEffect(() => () => CORE_PXL?.pxl_puck_clear(), []);

  return null;
}

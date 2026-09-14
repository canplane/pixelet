// @pxl/web/module/world/marker/debug_layer.tsx

/**
 * The dev gizmos (the `show_debug_objects` setting): the cell the map
 * centre floors to, the map centre itself, the floating origin, and the
 * page's base and centre corners, as line segments the module holds and
 * the renderer draws over everything (crates/pxl-core/src/debug.rs,
 * render/gpu/debug.ts), with labels as DOM over the canvas, projected
 * through the view each frame.
 */

import { useEffect, useMemo, useRef, useState } from "react";

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { mat4_clip_w, mat4_transform_point } from "@xpute/core/kit/mat4.ts";
import { type IVec, ivec, projection, type PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L1_IDX_IN_L4, L3_BASE, L3_CENTER } from "@pxl/shared/spec/space/indexer.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";
import { fill3, floor3n, type V3 } from "@pxl/shared/spec/space/v3.ts";
import { U_PER_CELL } from "@pxl/shared/bake/constant.ts";

import { get_ele, with_ele } from "@/module/world/terrain/elevation.ts";
import { register_tile_interest } from "@/module/world/terrain/store.ts";
import { useTick } from "@/module/world/sched/tick.ts";
import { request_world_tick } from "@/module/world/change.ts";
import { view } from "@/module/world/view.ts";
import { CORE_PXL, FIXED_ALLOC } from "@/render/layer/pipeline/core_pool.ts";

const COLOR: V3 = [0, 0, 0];
const DASH: f64 = 0.3;
const GAP: f64 = 0.2;
/** Half the cross a point is drawn as, in units. */
const POINT_HALF: f64 = 0.15;
const VERTEX_FLOATS = 6;

type Label = { at: V3; text: string };

/** A sketch: segments as pairs of vertices, and labels at points. */
class Sketch {
  readonly vertices: number[] = [];
  readonly labels: Label[] = [];

  line(from: V3, to: V3): void {
    this.vertices.push(from[0], from[1], from[2], ...COLOR, to[0], to[1], to[2], ...COLOR);
  }

  dashed(from: V3, to: V3): void {
    const len = Math.hypot(to[0] - from[0], to[1] - from[1], to[2] - from[2]);
    if (len === 0) return;
    const d: V3 = [(to[0] - from[0]) / len, (to[1] - from[1]) / len, (to[2] - from[2]) / len];
    for (let s = 0; s < len; s += DASH + GAP) {
      const e = Math.min(len, s + DASH);
      this.line([from[0] + d[0] * s, from[1] + d[1] * s, from[2] + d[2] * s], [from[0] + d[0] * e, from[1] + d[1] * e, from[2] + d[2] * e]);
    }
  }

  point(at: V3, label?: string): void {
    for (const axis of [0, 1, 2]) {
      const a: V3 = [...at], b: V3 = [...at];
      a[axis] -= POINT_HALF;
      b[axis] += POINT_HALF;
      this.line(a, b);
    }
    if (label) this.labels.push({ at, text: label });
  }

  box(base: V3, span: f64, label?: string): void {
    const c = (x: number, y: number, z: number): V3 => [base[0] + x * span, base[1] + y * span, base[2] + z * span];
    const corners = [c(0, 0, 0), c(1, 0, 0), c(1, 1, 0), c(0, 1, 0), c(0, 0, 1), c(1, 0, 1), c(1, 1, 1), c(0, 1, 1)];
    for (const [a, b] of [[0, 1], [1, 2], [2, 3], [3, 0], [4, 5], [5, 6], [6, 7], [7, 4], [0, 4], [1, 5], [2, 6], [3, 7]]) this.line(corners[a], corners[b]);
    const centre = c(0.5, 0.5, 0.5);
    this.labels.push({ at: c(0.5, 0, 0), text: String(span) });
    if (label) this.labels.push({ at: centre, text: label });
  }

  labelled_line(from: V3, to: V3, label?: string, dashed = false): void {
    if (dashed) this.dashed(from, to);
    else this.line(from, to);
    if (label) this.labels.push({ at: [(from[0] + to[0]) / 2, (from[1] + to[1]) / 2, (from[2] + to[2]) / 2], text: label });
  }
}

/** A point in the root frame: its address's offset from the map origin. */
function local_of(p: PVF3, map_origin: IVec): V3 {
  const local = ivec.sub(p.i, map_origin);
  const v = projection.unproject(fill3(0), 0n, p.f);
  return [ivec.to_real(ivec.X(local)) + v[0], ivec.to_real(ivec.Y(local)) + v[1], ivec.to_real(ivec.Z(local)) + v[2]];
}

function sketch(map_center: PVF3, map_origin: IVec): Sketch {
  const s = new Sketch();

  // raw_v: the map centre as placed, no elevation override. quant_v: the
  // cell it floors to, ground-snapped — a cell, so a box.
  const raw_v: V3 = projection.unproject(fill3(0), L1_IDX_IN_L4(map_center.i), map_center.f);
  const gnd_v: V3 = [raw_v[0], raw_v[1], get_ele(map_center.i, map_center.f) ?? 0];
  const quant_v: V3 = floor3n(gnd_v, U_PER_CELL);

  // The floating origin, grounded and floored: local (0, 0, ground).
  const anchor_base_gnd_v: V3 = projection.unproject(fill3(0), L1_IDX_IN_L4(map_origin), 0n);
  anchor_base_gnd_v[2] = get_ele(map_origin, 0n) ?? 0;
  const anchor_base_quant_v: V3 = floor3n(anchor_base_gnd_v, U_PER_CELL);

  s.box(quant_v, U_PER_CELL, "floor(map_center)");
  s.point(anchor_base_quant_v, "anchor_base (map_origin)");
  s.labelled_line(quant_v, raw_v);
  s.labelled_line(quant_v, anchor_base_quant_v, "floor(map_center) <-> anchor_base (map_origin)", true);

  for (const [corner, label] of [[TO_XY(L3_BASE(map_center.i)), "page_base"], [TO_XY(L3_CENTER(map_center.i)), "page_center"]] as const) {
    const grounded = with_ele({ i: 0n, f: 0n }, corner, 0n);
    if (grounded) s.box(local_of(grounded, map_origin), 1, label);
  }
  s.point(raw_v, "map_center");
  return s;
}

type DebugLayerProps = {
  map_center: PVF3;
  map_origin: IVec;
};

export function DebugLayer(props: DebugLayerProps) {
  const { map_center, map_origin } = props;
  const [tiles, set_tiles] = useState(0);
  useEffect(() => register_tile_interest(() => true, () => set_tiles((t) => t + 1)), []);

  const drawn = useMemo(() => sketch(map_center, map_origin), [map_center, map_origin, tiles]);

  useEffect(() => {
    const p = CORE_PXL;
    if (!p) return;
    const count: u32 = drawn.vertices.length / VERTEX_FLOATS;
    const lane = FIXED_ALLOC.take(Float32Array, Math.max(VERTEX_FLOATS, drawn.vertices.length));
    lane.set(drawn.vertices);
    p.pxl_debug_set(ivec.to_real(ivec.X(map_origin)), ivec.to_real(ivec.Y(map_origin)), ivec.to_real(ivec.Z(map_origin)), count, lane.byteOffset);
    FIXED_ALLOC.give(lane);
    request_world_tick();
  }, [drawn, map_origin]);
  // Gone with the layer.
  useEffect(() => () => CORE_PXL?.pxl_debug_set(0, 0, 0, 0, 0), []);

  const refs = useRef<(HTMLDivElement | null)[]>([]);
  const $p = useRef({ x: 0, y: 0, z: 0 }).current;
  useTick("cosmetic", () => {
    drawn.labels.forEach((label, k) => {
      const el = refs.current[k];
      if (!el) return;
      const [x, y, z] = label.at;
      const behind = mat4_clip_w(view.local_view_proj, x, y, z) <= 0;
      mat4_transform_point($p, view.local_view_proj, x, y, z);
      el.hidden = behind;
      el.style.transform = `translate(-50%, -50%) translate(${(($p.x + 1) / 2) * view.width}px, ${((1 - $p.y) / 2) * view.height}px)`;
    });
  }, "debug_labels");

  return (
    <div className="pointer-events-none absolute inset-0 overflow-hidden">
      {drawn.labels.map((label, k) => (
        <div
          key={`${k}:${label.text}`}
          ref={(el) => {
            refs.current[k] = el;
          }}
          className="absolute left-0 top-0 whitespace-nowrap text-[10px] text-black"
        >
          {label.text}
        </div>
      ))}
    </div>
  );
}

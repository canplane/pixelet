// @pxl/web/module/world/marker/debug_gizmos.tsx

/**
 * The dev gizmos (the `show_debug_objects` setting): the cell the map
 * centre floors to, the map centre itself, the floating origin, the page's
 * base and centre corners, and the two frames' gyro rings — one fixed to
 * the screen whatever the azimuth, one turning with the map so its vertex
 * points north — as line segments the module holds and the renderer draws
 * over everything (crates/pxl/core/src/world/debug.rs, render/gpu/debug.ts),
 * with labels as DOM over the canvas, projected through the view each
 * frame. The map's part is rebuilt when the map moves, the frames' part
 * when the view does; both go to the module as one set.
 */

import { useEffect, useMemo, useRef, useState } from "react";

import type { f64 } from "@xpute/core/kit/word.ts";
import { type Mat4, mat4_clip_w, mat4_transform_point } from "@xpute/core/kit/mat4.ts";
import { type IVec, ivec, projection, type PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L1_IDX_IN_L4, L3_BASE, L3_CENTER } from "@pxl/shared/spec/space/indexer.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";
import { fill3, floor3n, type V3 } from "@pxl/shared/spec/space/v3.ts";
import { U_PER_CELL } from "@pxl/shared/bake/constant.ts";

import { get_ele, with_ele } from "@/module/world/terrain/elevation.ts";
import { register_tile_interest } from "@/module/world/terrain/store.ts";
import { useTick } from "@/module/world/sched/tick.ts";
import { request_world_tick } from "@/module/world/change.ts";
import { view } from "@/module/world/frame/view.ts";
import { CORE_PXL } from "@/wasm/core_pool.ts";
import { CORE_BUILD, packet } from "@/wasm/core_build.ts";
import { Cmd } from "@pxl/shared/spec/cmd.ts";

const COLOR: V3 = [0, 0, 0];
const DASH: f64 = 0.3;
const GAP: f64 = 0.2;
/** Half the cross a point is drawn as, in units. */
const POINT_HALF: f64 = 0.15;
const RING_RADIUS: f64 = 4;
const RING_SEGMENTS = 64;

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

  /** A dashed ring of `radius` about `centre` in the plane of `a` and `b`. */
  ring(centre: V3, radius: f64, a: V3, b: V3): void {
    let prev: V3 | null = null;
    for (let i = 0; i <= RING_SEGMENTS; i++) {
      const t = (i / RING_SEGMENTS) * Math.PI * 2;
      const c = Math.cos(t) * radius, sn = Math.sin(t) * radius;
      const at: V3 = [centre[0] + a[0] * c + b[0] * sn, centre[1] + a[1] * c + b[1] * sn, centre[2] + a[2] * c + b[2] * sn];
      if (prev && i % 2 === 1) this.line(prev, at);
      prev = at;
    }
  }

  /**
   * A gyro ring: a horizontal ring and a vertical one
   * through `vertex`, the vertex marked, and the radius drawn when asked,
   * all in the frame `to_local` maps into the root frame.
   */
  gyro(to_local: (v: V3) => V3, vertex: V3, label: string, show_radius: boolean): void {
    const horiz = Math.hypot(vertex[0], vertex[1]);
    const dir: V3 = horiz > 1e-9 ? [vertex[0] / horiz, vertex[1] / horiz, 0] : [1, 0, 0];
    const radius = Math.hypot(vertex[0], vertex[1], vertex[2]);
    const centre = to_local([0, 0, 0]);
    const axis = (v: V3): V3 => {
      const p = to_local(v);
      return [p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]];
    };
    this.ring(centre, radius, axis([1, 0, 0]), axis([0, 1, 0]));
    this.ring(centre, radius, axis(dir), axis([0, 0, 1]));
    if (show_radius) this.labelled_line(centre, to_local(vertex), String(radius), true);
    this.point(to_local(vertex), label);
  }

  append(other: Sketch): void {
    this.vertices.push(...other.vertices);
    this.labels.push(...other.labels);
  }
}

const $v = { x: 0, y: 0, z: 0 };
/** A point of a matrix's frame, in the root frame. */
const through = (m: Mat4) => (v: V3): V3 => {
  mat4_transform_point($v, m, v[0], v[1], v[2]);
  return [$v.x, $v.y, $v.z];
};

/** The frames' rings: the view frame, which never turns, and the world
 * frame, which turns with the map, both about the view's origin. */
function sketch_frames(s: Sketch): void {
  const view_frame = through(view.inv_root);
  s.gyro(view_frame, [4, 0, 0], "view-frame (4,0,0)", false);
  s.point(view_frame([0, 0, 4]), "view-frame (0,0,4)");
  const world_frame = (v: V3): V3 => [v[0] - view.tx, v[1] - view.ty, v[2] - view.tz];
  s.gyro(world_frame, [0, RING_RADIUS, 0], "north (world-frame, azimuth-locked)", true);
}

/** A point in the root frame: its address's offset from the map origin. */
function local_of(p: PVF3, map_origin: IVec): V3 {
  const local = ivec.sub(p.i, map_origin);
  const v = projection.unproject(fill3(0), 0n, p.f);
  return [ivec.to_real(ivec.X(local)) + v[0], ivec.to_real(ivec.Y(local)) + v[1], ivec.to_real(ivec.Z(local)) + v[2]];
}

function sketch_map(map_center: PVF3, map_origin: IVec): Sketch {
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

type DebugGizmosProps = {
  map_center: PVF3;
  map_origin: IVec;
};

export function DebugGizmos(props: DebugGizmosProps) {
  const { map_center, map_origin } = props;
  const [tiles, set_tiles] = useState(0);
  useEffect(() => register_tile_interest(() => true, () => set_tiles((t) => t + 1)), []);

  const map_sketch = useMemo(() => sketch_map(map_center, map_origin), [map_center, map_origin, tiles]);
  const drawn = useRef<Sketch>(new Sketch());
  const [label_texts, set_label_texts] = useState<string[]>([]);
  const built_for = useRef<{ map: Sketch | null; key: string }>({ map: null, key: "" });

  // Gone with the layer.
  useEffect(() => () => CORE_BUILD.send(0, Cmd.DEBUG_UPDATE, packet((b) => b.f64(0).f64(0).f64(0).f32_array(new Float32Array(0)))), []);

  const refs = useRef<(HTMLDivElement | null)[]>([]);
  const $p = useRef({ x: 0, y: 0, z: 0 }).current;
  useTick("cosmetic", () => {
    const p = CORE_PXL;
    if (!p) return;
    // The frames' rings follow the view; the map's part the map.
    const key = `${view.azimuth}|${view.tx}|${view.ty}|${view.tz}`;
    if (built_for.current.map !== map_sketch || built_for.current.key !== key) {
      built_for.current = { map: map_sketch, key };
      const s = new Sketch();
      s.append(map_sketch);
      sketch_frames(s);
      drawn.current = s;
      const vertices = Float32Array.from(s.vertices);
      CORE_BUILD.send(0, Cmd.DEBUG_UPDATE, packet((b) => b.f64(ivec.to_real(ivec.X(map_origin))).f64(ivec.to_real(ivec.Y(map_origin))).f64(ivec.to_real(ivec.Z(map_origin))).f32_array(vertices)));
      request_world_tick();
      const texts = s.labels.map((l) => l.text);
      if (texts.join("\n") !== label_texts.join("\n")) set_label_texts(texts);
    }
    drawn.current.labels.forEach((label, k) => {
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
      {label_texts.map((text, k) => (
        <div
          key={`${k}:${text}`}
          ref={(el) => {
            refs.current[k] = el;
          }}
          className="absolute left-0 top-0 whitespace-nowrap text-[10px] text-black"
        >
          {text}
        </div>
      ))}
    </div>
  );
}

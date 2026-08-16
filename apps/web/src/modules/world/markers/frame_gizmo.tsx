// @pxl/web/modules/world/markers/frame_gizmo.tsx

/** Dev-only gyroscope-style debug markers for World's frame indicators (__IS_DEV__ + show_debug_objects gated by every caller). */

import { useLayoutEffect, useRef } from "react";
import { Html, Line } from "@react-three/drei";
import type { Line2 } from "three-stdlib";

import { center3, type V3 } from "@pxl/shared/spec/space/v3.ts";

/**
 * One shared look for every debug marker in this file (rings, lines, box
 * edges, point vertices) — a single color and stroke width instead of the
 * green/yellow/red/blue/black/orange this used to carry per-marker. Markers
 * are told apart by their label now, not by color, so the whole overlay
 * reads as one consistent diagnostic layer instead of a set of unrelated
 * colored shapes.
 */
const DEBUG_COLOR = "#000000";
const DEBUG_LINE_WIDTH = 1.5;
/** THREE.PointsMaterial size in px (sizeAttenuation off — constant on-screen size regardless of distance, same as Html labels already are via distanceFactor). */
const DEBUG_POINT_PX = 6;
/**
 * depthTest off + a render order past anything real world content ever
 * uses is this scene's equivalent of a CSS z-index: these are diagnostic
 * overlays, not real geometry, so terrain/objects drawn in front of one in
 * the depth buffer should never hide it.
 */
const DEBUG_RENDER_ORDER = 9999;

type LineProps = Omit<React.ComponentProps<typeof Line>, "ref">;

/**
 * drei's Line always renders its very first frame with the underlying
 * LineMaterial's own construction-time default (fog enabled) before this
 * component's `fog={false}` prop has actually landed on the material —
 * visible as a flash of scene-fog-blended (near-white against this scene's
 * sky) color for exactly one frame, self-correcting on any later re-render
 * (e.g. toggling show_debug_objects off/on), which is why only the very
 * first mount ever showed it. Grabbing the underlying Line2 in a
 * layout-effect and forcing `material.fog = false` + `needsUpdate` makes
 * the correction land before the browser paints the first frame, instead
 * of relying on prop-application timing.
 */
function FoglessLine(props: LineProps) {
  const ref = useRef<Line2>(null);
  useLayoutEffect(() => {
    const material = ref.current?.material;
    if (!material) return;
    material.fog = false;
    material.depthTest = false;
    material.needsUpdate = true;
  });
  return <Line ref={ref} fog={false} depthTest={false} renderOrder={DEBUG_RENDER_ORDER} {...props} />;
}

function debug_label(position: V3, label?: string) {
  if (!label) return null;
  return (
    <Html position={position} center distanceFactor={12} style={{ pointerEvents: "none", color: DEBUG_COLOR, fontSize: 10, whiteSpace: "nowrap" }}>
      {label}
    </Html>
  );
}

type DebugPointProps = {
  position: V3;
  label?: string;
};

/**
 * A true point — THREE.Points, not a small sphere mesh — so a vertex marker
 * has no implied volume/geometry and reads as "exactly this coordinate",
 * not "a tiny ball placed here".
 */
export function DebugPoint(props: DebugPointProps) {
  const { position, label } = props;
  return (
    <group name="debug-point">
      <points position={position} renderOrder={DEBUG_RENDER_ORDER}>
        <bufferGeometry>
          <bufferAttribute attach="attributes-position" args={[new Float32Array([0, 0, 0]), 3]} />
        </bufferGeometry>
        <pointsMaterial color={DEBUG_COLOR} size={DEBUG_POINT_PX} sizeAttenuation={false} fog={false} depthTest={false} />
      </points>
      {debug_label(position, label)}
    </group>
  );
}

type GyroRingProps = {
  /** Ring radius — matches the distance from center to `vertex`, so the vertex marker sits exactly on the ring. */
  radius: number;
  /** Where this frame's original point marker used to sit — kept as a labeled vertex where both rings cross. */
  vertex: V3;
  label: string;
  /** When true, draws a dashed center-to-vertex spoke labeled with just the radius number — same convention as DebugBox's edge-length label — opt-in per caller rather than automatic on every ring. */
  show_radius?: boolean;
};

function ring_points(radius: number, basis_a: [number, number, number], basis_b: [number, number, number]): V3[] {
  const segments = 64;
  return Array.from({ length: segments + 1 }, (_, i) => {
    const a = (i / segments) * Math.PI * 2;
    const c = Math.cos(a) * radius, s = Math.sin(a) * radius;
    return [
      basis_a[0] * c + basis_b[0] * s,
      basis_a[1] * c + basis_b[1] * s,
      basis_a[2] * c + basis_b[2] * s,
    ];
  });
}

/**
 * Two perpendicular rings crossing at `vertex` and its antipode — an actual
 * gyroscope-gimbal shape, not a single flat ring with a floating point. Both
 * dashed (a solid/dashed split was tried and dropped — dashing both reads as
 * one consistent ring family rather than singling one out). The horizontal
 * ring lies in the local XY plane; the vertical ring lies in the plane
 * spanned by the horizontal direction toward `vertex` and the Z axis, so it
 * necessarily passes back through `vertex` — that shared point is where the
 * two rings actually intersect, not just visually overlap.
 */
export function GyroRing(props: GyroRingProps) {
  const { radius, vertex, label, show_radius } = props;
  const horiz_len = Math.hypot(vertex[0], vertex[1]);
  const dir: [number, number, number] = horiz_len > 1e-9 ? [vertex[0] / horiz_len, vertex[1] / horiz_len, 0] : [1, 0, 0];

  const horizontal = ring_points(radius, [1, 0, 0], [0, 1, 0]);
  const vertical = ring_points(radius, dir, [0, 0, 1]);

  return (
    <group name={`gyro-ring-${label}`}>
      <FoglessLine points={horizontal} color={DEBUG_COLOR} lineWidth={DEBUG_LINE_WIDTH} dashed dashSize={0.3} gapSize={0.2} />
      <FoglessLine points={vertical} color={DEBUG_COLOR} lineWidth={DEBUG_LINE_WIDTH} dashed dashSize={0.3} gapSize={0.2} />
      {show_radius && <LabeledLine from={[0, 0, 0]} to={vertex} label={String(radius)} dashed />}
      <DebugPoint position={vertex} label={label} />
    </group>
  );
}

type DebugBoxProps = {
  base: V3;
  span: number;
  label?: string;
};

const BOX_EDGES: readonly [V3, V3][] = (() => {
  const c: V3[] = [
    [0, 0, 0],
    [1, 0, 0],
    [1, 1, 0],
    [0, 1, 0],
    [0, 0, 1],
    [1, 0, 1],
    [1, 1, 1],
    [0, 1, 1],
  ];
  const pairs: [number, number][] = [
    [0, 1],
    [1, 2],
    [2, 3],
    [3, 0],
    [4, 5],
    [5, 6],
    [6, 7],
    [7, 4],
    [0, 4],
    [1, 5],
    [2, 6],
    [3, 7],
  ];
  return pairs.map(([a, b]) => [c[a], c[b]]);
})();

/**
 * Line-drawn box edges (12 unit-cube edges scaled/translated to base+span) —
 * no fill, no lighting, no shadow, and the same Line primitive as every
 * other marker here — deliberately NOT the solid, lit, shadow-casting look
 * a real placed object uses (chunk_stage.tsx's OBJECT_BOX instances), so a
 * debug marker never reads as "an actual object placed here" at a glance.
 */
export function DebugBox(props: DebugBoxProps) {
  const { base, span, label } = props;
  const center = center3(base, [span, span, span]);
  const edge_mid: V3 = [0, -0.5 * span, -0.5 * span];
  return (
    <group name="debug-box" position={center}>
      {BOX_EDGES.map(([a, b], i) => {
        const from: V3 = [(a[0] - 0.5) * span, (a[1] - 0.5) * span, (a[2] - 0.5) * span];
        const to: V3 = [(b[0] - 0.5) * span, (b[1] - 0.5) * span, (b[2] - 0.5) * span];
        return <FoglessLine key={i} points={[from, to]} color={DEBUG_COLOR} lineWidth={DEBUG_LINE_WIDTH} />;
      })}
      {/* edge-length readout on the bottom-front edge (BOX_EDGES[0], the (0,0,0)-(1,0,0) edge) */}
      {debug_label(edge_mid, String(span))}
      {debug_label([0, 0, 0], label)}
    </group>
  );
}

type LabeledLineProps = {
  from: V3;
  to: V3;
  /** Optional — omit when the two endpoints' own labels already make the connection self-explanatory. */
  label?: string;
  /** Dashed reads as a measurement/annotation rather than a structural edge. */
  dashed?: boolean;
};

/** A plain segment, optionally labeled at its midpoint — for visualizing the gap between two debug points that are conceptually "the same" point (quantized vs. true, frame origin vs. a fixed landmark, ...). */
export function LabeledLine(props: LabeledLineProps) {
  const { from, to, label, dashed = false } = props;
  const mid: V3 = [(from[0] + to[0]) / 2, (from[1] + to[1]) / 2, (from[2] + to[2]) / 2];
  return (
    <group name={`labeled-line-${label ?? "unlabeled"}`}>
      <FoglessLine
        points={[from, to]}
        color={DEBUG_COLOR}
        lineWidth={DEBUG_LINE_WIDTH}
        dashed={dashed}
        dashSize={dashed ? 0.3 : undefined}
        gapSize={dashed ? 0.2 : undefined}
      />
      {debug_label(mid, label)}
    </group>
  );
}

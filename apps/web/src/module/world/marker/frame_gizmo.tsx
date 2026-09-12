// @pxl/web/module/world/marker/frame_gizmo.tsx

import { useLayoutEffect, useRef } from "react";
import { Html, Line } from "@react-three/drei";
import type { Line2 } from "three-stdlib";

import { center3, type V3 } from "@pxl/shared/spec/space/v3.ts";

const DEBUG_COLOR = "#000000";
const DEBUG_LINE_WIDTH = 1.5;
const DEBUG_POINT_PX = 6;
const DEBUG_RENDER_ORDER = 9999;

type LineProps = Omit<React.ComponentProps<typeof Line>, "ref">;

// drei's Line renders its very first frame with LineMaterial's construction-time
// default (fog enabled) before this component's `fog={false}` prop lands on the
// material — a one-frame flash of fog-blended color on first mount. Grabbing
// Line2 in a layout effect and forcing `material.fog = false` there lands the
// correction before paint instead of relying on prop-application timing.
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
  radius: number;
  vertex: V3;
  label: string;
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
      {debug_label(edge_mid, String(span))}
      {debug_label([0, 0, 0], label)}
    </group>
  );
}

type LabeledLineProps = {
  from: V3;
  to: V3;
  label?: string;
  dashed?: boolean;
};

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

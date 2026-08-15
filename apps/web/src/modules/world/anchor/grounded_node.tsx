// @pxl/web/modules/world/anchor/grounded_node.tsx

/**
 * Ground-relative anchor node.
 *
 * Semantics:
 * - X/Y are used to sample terrain height
 * - input Z is interpreted as local height relative to that terrain
 * - final Z = input Z + sampled ground height
 *
 * Dispatch is delegated to AnchorNode.
 */

import type { ReactNode } from "react";

import { type FVec, type IVec, type PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { with_ele } from "../terrain/elevation.ts";
import { AnchorNode } from "./node.tsx";

type GroundedNodeProps = {
  /** canonical PVF3 coordinate; X/Y are world position, Z is ground-relative local height */
  i: IVec;
  f?: FVec;

  children: ReactNode;
};
export function GroundedNode(props: GroundedNodeProps) {
  const { i, f = 0n, children } = props;

  const grounded: PVF3 | null = with_ele({ i: 0n, f: 0n }, i, f);
  if (grounded === null) return null;

  // dispatch through the anchor portal.
  return (
    <AnchorNode i={grounded.i} f={grounded.f}>
      {children}
    </AnchorNode>
  );
}

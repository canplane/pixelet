// @pxl/web/modules/world/markers/cell-highlighter/object.ts

import * as THREE from "three/webgpu";
import { color, float, fwidth, mix, oneMinus, smoothstep, uv } from "three/tsl";

import type { PVF3 } from "@pxl/spec/space/pvf3/mod.ts";
import { ivec, pvf3 } from "@pxl/spec/space/pvf3/mod.ts";
import { L4_BASE, L4_IDX } from "@pxl/spec/space/indexer.ts";
import { center3n } from "@pxl/spec/space/v3.ts";

import { U_PER_CELL } from "@pxl/bake/terrain/constants.ts";

import type { AnchorRegistry } from "../../spatial/anchor/registry.ts";

// Highlighter style
const highlight_style = {
  COLOR: "#ff9f05",
  FILL_OPACITY: .6,
  BORDER_WIDTH: .05,
};

const HIGHLIGHT_BOX = new THREE.BoxGeometry(1, 1, 1);

const POS = center3n([0, 0, -U_PER_CELL], U_PER_CELL);

function create_material(): THREE.MeshBasicNodeMaterial {
  const material = new THREE.MeshBasicNodeMaterial();

  const v_uv = uv();
  const border = float(highlight_style.BORDER_WIDTH);
  const fill_opacity = float(highlight_style.FILL_OPACITY);

  function aastep(thres: any, value: any) {
    const afwidth = fwidth(value).mul(.5);
    return smoothstep(thres.sub(afwidth), thres.add(afwidth), value);
  }

  const bx = aastep(border, v_uv.x).mul(aastep(v_uv.x, oneMinus(border)));
  const by = aastep(border, v_uv.y).mul(aastep(v_uv.y, oneMinus(border)));

  const is_center = bx.mul(by);
  const is_border = oneMinus(is_center);

  const alpha = mix(fill_opacity, float(1), is_border);

  material.colorNode = color(highlight_style.COLOR);
  material.opacityNode = alpha;

  material.transparent = true;
  material.side = THREE.DoubleSide;
  material.depthTest = true;
  material.polygonOffset = true;
  material.polygonOffsetFactor = -2.0;
  material.polygonOffsetUnits = -8.0;

  return material;
}

/**
 * CellHighlighterObject
 *
 * Renderer-owned highlighter object.
 * - owns mesh/material lifetime
 * - binds itself to a live L4 anchor group
 * - applies anchor-local PVF3 offset
 *
 * NOTE:
 * This is intentionally React-free.
 * React should only drive visibility / target updates from the control plane.
 */
export class CellHighlighterObject {
  readonly root = new THREE.Group();

  readonly mesh: THREE.Mesh<THREE.BoxGeometry, THREE.MeshBasicNodeMaterial>;
  readonly material: THREE.MeshBasicNodeMaterial;

  private registry: AnchorRegistry;
  private anchor_idx: bigint | null = null;

  constructor(registry: AnchorRegistry) {
    this.registry = registry;

    this.material = create_material();

    this.mesh = new THREE.Mesh(HIGHLIGHT_BOX, this.material);
    this.mesh.position.set(POS[0], POS[1], POS[2]);
    this.mesh.scale.setScalar(U_PER_CELL);
    this.mesh.renderOrder = 9999;

    this.root.visible = false;
    this.root.add(this.mesh);
  }

  set_coord(coord: PVF3 | null): void {
    if (coord === null) {
      this._detach_anchor();
      this.root.visible = false;
      return;
    }

    const next_anchor_idx = L4_IDX(coord.i);

    if (this.anchor_idx !== next_anchor_idx) {
      this._rebind_anchor(next_anchor_idx);
    }

    const anchor = L4_BASE(coord.i);
    const local = { i: ivec.sub(coord.i, anchor), f: coord.f };

    this.root.position.set(
      pvf3.to_real(pvf3.X(local)),
      pvf3.to_real(pvf3.Y(local)),
      pvf3.to_real(pvf3.Z(local)),
    );
    this.root.updateMatrix();

    this.root.visible = true;
  }

  dispose(): void {
    this._detach_anchor();
    this.root.clear();
    this.material.dispose();
  }

  private _rebind_anchor(next_anchor_idx: bigint): void {
    if (this.anchor_idx === next_anchor_idx) return;

    this._detach_anchor();

    const group = this.registry.link(next_anchor_idx);
    group.add(this.root);

    this.anchor_idx = next_anchor_idx;
  }

  private _detach_anchor(): void {
    if (this.anchor_idx === null) return;

    this.root.removeFromParent();
    this.registry.unlink(this.anchor_idx);
    this.anchor_idx = null;
  }
}

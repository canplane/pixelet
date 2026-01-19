// @xpute/core/rpc/peer/role_index.ts

import { type Comp, comp_str } from "@xpute/spec/lib/kit/comp.ts";
import { TreapSet } from "@xpute/spec/lib/ds/treap_set.ts";
import { derive_seed_u64 } from "@xpute/spec/lib/kit/rng.ts";

import { type NodeSelector, role_indices } from "@xpute/spec/rpc/node.ts";
import type { PID } from "@xpute/spec/net/crypto.ts";

const ROLE_BITS = 63;

const ROLE_TAG_SALT = 0x243f6a8885a308d3n;
const role_tag = (r: number): bigint => (BigInt(r) + 1n) * ROLE_TAG_SALT;

/**
 * RoleIndex: member-count mass bookkeeping
 */
export class RoleIndex {
  private readonly roles_of = new Map<PID, number[]>();
  private readonly share_of = new Map<PID, number>(); // 1/M

  // role mass = Σ(1/M)
  private readonly mass = new Float64Array(ROLE_BITS);

  readonly lut: TreapSet<PID>[];

  constructor(balance_seed: bigint) {
    const comp: Comp<PID> = comp_str; // sampling base is always pid-order
    this.lut = Array.from({ length: ROLE_BITS }, (_, r) => {
      return new TreapSet<PID>(comp, derive_seed_u64(balance_seed, role_tag(r)));
    });
  }

  // weighted role pick support
  role_mass(r: number): number {
    return this.mass[r] ?? 0;
  }

  register(pid: PID, role_mask: NodeSelector): void {
    const old = this.roles_of.get(pid);
    if (old) {
      const oldShare = this.share_of.get(pid) ?? (old.length ? 1 / old.length : 0);
      for (let i = 0; i < old.length; i++) {
        const k = old[i];
        this.lut[k].delete(pid); // FIX: remove from old role set
        this.mass[k] -= oldShare;
      }
      this.roles_of.delete(pid);
      this.share_of.delete(pid);
    }

    const roles = role_indices(role_mask);
    if (roles.length === 0) return;

    this.roles_of.set(pid, roles);
    const share = 1 / roles.length;
    this.share_of.set(pid, share);

    for (let i = 0; i < roles.length; i++) {
      const k = roles[i];
      this.lut[k].insert(pid);
      this.mass[k] += share;
    }
  }

  unregister(pid: PID): void {
    const roles = this.roles_of.get(pid);
    if (!roles) return;

    this.roles_of.delete(pid);
    const share = this.share_of.get(pid) ?? (roles.length ? 1 / roles.length : 0);
    this.share_of.delete(pid);

    for (let i = 0; i < roles.length; i++) {
      const k = roles[i];
      this.lut[k].delete(pid);
      this.mass[k] -= share;
    }
  }
}

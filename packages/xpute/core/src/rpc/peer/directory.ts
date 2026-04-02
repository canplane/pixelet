// @xpute/core/rpc/peer/directory.ts

/**
 * peer_directory.ts — PeerDirectory (membership + selector-based resolution)
 *
 * Defines PeerDirectory, the peer registry used by the RPC routing layer.
 *
 * Responsibilities:
 *  - Membership tracking (register/unregister peers)
 *  - ID-mode resolution: VID -> PID translation via NAT
 *  - ROLE-mode resolution: union-of-roles selection with global rank-mix ordering
 *
 * Selector modes:
 *
 *  1) ID-mode (unicast fast path)
 *     - Selector encodes a directory-local virtual id (VID).
 *     - NAT translates VID -> PID.
 *
 *  2) ROLE-mode (anycast / multicast)
 *     - Selector encodes an "any-of roles" query (union of role sets).
 *     - Selection is delegated to role_select.ts.
 *
 * ROLE selection model:
 *  - Each candidate PID has a scalar score: score(pid) = score_of(pid).
 *  - mix ∈ [0..1] blends score-driven routing and deterministic randomness:
 *
 *      rank(pid) = mix * score(pid) + (1 - mix) * U(seed) + eps(seed)
 *
 *    where U(seed) is deterministic uniform noise derived from seed and eps breaks ties.
 *    Higher rank appears earlier in the output.
 *
 * Output contract (ROLE-mode):
 *  - Output contains unique PIDs only (sampling without replacement).
 *  - Output is max-fanout:
 *      - if opts.fanout is provided: out.len <= min(fanout, n)
 *      - if opts.fanout is omitted: treated as fanout = n (bounded by membership size)
 *  - MAY return fewer than k:
 *      - if the reachable unique candidate universe is smaller than fanout
 *      - or if the internal PARTIAL candidate generator stalls early (policy-dependent)
 *
 * Determinism:
 *  - If opts.seed is provided, selection is deterministic given:
 *      - identical membership snapshot
 *      - identical selector/roles query
 *      - identical (seed, mix, fanout)
 *      - identical role_index traversal behavior (roles[] ordering + pool enumeration order)
 *  - If opts.seed is omitted, seed64() is used and output becomes non-deterministic.
 *
 * Notes:
 *  - Tree balancing determinism is controlled separately via balance_seed (RoleIndex/TreapSet).
 *  - Strict equal-probability sampling is NOT a goal; this is a routing heuristic.
 */

import { seed64 } from "@xpute/spec/lib/kit/rng.ts";
import { CLAMP } from "@xpute/spec/lib/kit/comp.ts";

import { FANOUT, IS_ROLE_SEL, type NodeSelector, role_indices } from "@xpute/spec/rpc/node.ts";
import type { PID } from "@xpute/spec/net/crypto.ts";
import type { Peer } from "@xpute/spec/net/xport/mod.ts";

import { RoleIndex } from "./role_index.ts";
import { NAT } from "./nat.ts";
import type { ScoreFn } from "./rankmix.ts";
import { resolve_roles } from "./role_select.ts";

/**
 * PeerDirectory: resolve_role with continuous mix
 */

/**
 * Optional parameters for PeerDirectory construction.
 *
 * @property score_of - Custom scoring function for peer selection
 *   - Higher score = preferred
 *   - Default: () => 0 (uniform randomness when mix < 1)
 *   - Example: (pid) => get_tier(pid) for QoS-based selection
 *
 * @property balance_seed - Seed for deterministic tree balancing
 *   - If provided: TreapSet shapes are reproducible
 *   - If undefined: Uses random entropy (non-deterministic)
 *   - Note: Does NOT affect resolve_role randomness (use resolve options seed)
 */
export type PeerDirectoryOptions = {
  score_of?: ScoreFn;
  balance_seed?: bigint;
};

export type ResolveOptions = {
  seed?: bigint;
  mix?: number; // 0..1
};

export class PeerDirectory {
  private readonly peers = new Map<PID, Peer>();

  private readonly nat: NAT;
  private readonly role_index: RoleIndex;

  readonly score_of: ScoreFn;
  readonly balance_seed: bigint;

  /**
   * @param opts - Optional configuration
   */
  constructor(opts: PeerDirectoryOptions = {}) {
    const { score_of = () => 0, balance_seed = seed64() } = opts;
    this.score_of = score_of;
    this.balance_seed = balance_seed;

    this.nat = new NAT();
    this.role_index = new RoleIndex(balance_seed);
  }

  // ---- internal membership ops ----

  register(peer: Peer): NodeSelector {
    this.peers.set(peer.pid, peer);
    this.role_index.register(peer.pid, peer.role_mask);
    return this.nat.register(peer.pid);
  }

  unregister(pid: PID): NodeSelector | undefined {
    this.peers.delete(pid);
    this.role_index.unregister(pid);
    return this.nat.unregister(pid);
  }

  reverse(pid: PID): [NodeSelector, Peer] | undefined {
    const peer: Peer | undefined = this.peers.get(pid);
    if (!peer) return undefined;

    const vid: NodeSelector = this.nat.pt.get(pid)!;
    return [vid, peer];
  }

  forward(sel: NodeSelector, fanout: number = FANOUT.ONE, opts: ResolveOptions = {}): Peer[] {
    return this._forward(sel, fanout, opts)
      .map((pid) => this.peers.get(pid))
      .filter((peer): peer is Peer => !!peer);
  }

  // ---------------------------------------------------------------------------

  private _forward(sel: NodeSelector, fanout: number, opts: ResolveOptions): PID[] {
    // Role-mode
    if (IS_ROLE_SEL(sel)) return this.resolve_roles(sel, fanout, opts);

    // ID-mode
    const pid = this.resolve_id(sel);
    return pid !== undefined ? [pid] : [];
  }

  // ID-mode selector -> PID
  private resolve_id(vid: NodeSelector): PID | undefined {
    return this.nat.vt.get(vid);
  }

  // ROLE-mode selector -> PID stream (deduped, max-k)
  private resolve_roles(role_mask: NodeSelector, fanout: number, opts: ResolveOptions): PID[] {
    const roles = role_indices(role_mask);
    if (!roles.length) return [];

    const n = this.peers.size;
    if (!n) return [];

    fanout = Math.min(fanout, n);
    if (fanout <= 0) return [];

    // normalize options
    const seed = opts.seed ?? seed64();
    const mix = CLAMP(opts.mix ?? 1, 0, 1);

    return resolve_roles(this.role_index, roles, n, seed, mix, fanout, this.score_of);
  }
}

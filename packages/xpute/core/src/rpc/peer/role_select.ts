// @xpute/core/rpc/peer/role_select.ts

/**
 * role_select.ts — Role-based peer selection policy (union-of-roles resolver)
 *
 * Implements ROLE-mode peer selection for PeerDirectory.
 *
 * Problem:
 *  - A selector may represent an "any-of roles" query (union of multiple role sets).
 *  - Nodes may belong to multiple roles (union can overrepresent multi-role nodes).
 *  - Naive inorder traversal introduces static prefix bias (low-id members favored).
 *  - We want a continuous knob (mix) between score preference and deterministic randomness,
 *    while keeping work bounded for small-k routing.
 *
 * Output contract:
 *  - Returns unique PIDs only (sampling without replacement).
 *  - Result is max-k: out.len <= k.
 *  - Ordering is derived from a GLOBAL rank-mix function:
 *
 *      rank(pid) = mix * score(pid) + (1 - mix) * U(seed) + eps(seed)
 *
 *    Higher rank MUST appear earlier.
 *
 * Determinism:
 *  - Deterministic given identical inputs:
 *      - identical RoleIndex snapshot (membership + role sets)
 *      - identical roles[] query ordering/content
 *      - identical (seed, mix, k)
 *      - identical policy parameters
 *      - identical candidate pool enumeration order (same traversal behavior)
 *
 * Algorithm:
 *
 *  1) FULL path (exact, higher cost)
 *     - Materialize the full union of unique candidates across roles.
 *     - Apply rankmix_full(): rank all, sort descending.
 *     - Cost:
 *         time  O(N log N)
 *         space O(N)
 *       where N is the number of unique candidates in the union.
 *
 *  2) PARTIAL path (approximation, bounded work)
 *     - Materialize a bounded candidate pool of size pool_lim:
 *
 *         pool_lim = min(n,
 *                        max(pool_floor(role_count),
 *                            ceil(k + C*sqrt(k))))
 *
 *       where C interpolates by mix:
 *         mix -> 1 : smaller pool is acceptable (score dominates)
 *         mix -> 0 : larger pool is required (randomness dominates)
 *
 *     - Candidate pooling (anti-prefix bias + dedupe):
 *         - per role: choose randomized start via kth(rand % size)
 *         - walk a bounded inorder window via iter_from(start, cap)
 *         - round-robin across role slots
 *         - global Set dedupe (no replacement across roles)
 *         - stop when pool_lim reached or all slots stall
 *
 *     - After pooling, apply rankmix_partial():
 *         - keep top-k by rank using a min-heap
 *         - output sorted by rank descending
 *
 *     - Cost (bounded by policy):
 *         time  O(pool_lim log k)
 *         space O(pool_lim + k)
 *
 * Role hedging (multi-role overrepresentation control):
 *  - PARTIAL pooling uses RoleIndex.role_mass(r) = Σ (1/M(pid)).
 *  - Window caps per role are adjusted using expected share and deficit feedback,
 *    to avoid a union-of-roles query being dominated by multi-role members.
 *
 * Policy:
 *  - RoleSelectPolicy is an internal knob:
 *      - FULL/PARTIAL cutoff ratio
 *      - pool floors and pool ~= k + C*sqrt(k) shaping range
 *  - Policy may change as long as the output contract holds.
 *
 * Non-goals:
 *  - Strict uniform probability sampling is NOT guaranteed.
 *  - This is a routing heuristic tuned for load spreading and practical cost bounds.
 */

import { make_rng } from "@xpute/spec/lib/kit/rng.ts";

import type { PID } from "@xpute/spec/net/crypto.ts";
import { RoleIndex } from "./role_index.ts";
import { rankmix_full, rankmix_partial, type ScoreFn } from "./rankmix.ts";

export type RoleSelectPolicy = {
  // FULL path threshold:
  // if k/n >= num/den then use FULL (rankmix over full candidate universe).
  full_cutoff_ratio_num: number;
  full_cutoff_ratio_den: number;

  // Minimum pool size regardless of k (prevents k=1 pathological narrowness).
  pool_abs_min: number;

  // Minimum pool size per role group:
  // pool_floor ~= pool_per_role_min * ceil(sqrt(role_count)).
  pool_per_role_min: number;

  // PARTIAL pool shaping:
  // pool ~= k + C*sqrt(k)
  // C interpolates by mix (score-driven => smaller C, random => larger C).
  pool_sqrt_k_c_min: number;
  pool_sqrt_k_c_max: number;
};

const DEFAULT_POLICY: RoleSelectPolicy = {
  full_cutoff_ratio_num: 1,
  full_cutoff_ratio_den: 3, // ~33%

  pool_abs_min: 8,
  pool_per_role_min: 4,
  pool_sqrt_k_c_min: 1,
  pool_sqrt_k_c_max: 3,
};

function choose_full(n: number, k: number, p: RoleSelectPolicy): boolean {
  // (k / n) >= (num / den)
  return k * p.full_cutoff_ratio_den >= n * p.full_cutoff_ratio_num;
}

function pool_floor(roles_len: number, p: RoleSelectPolicy): number {
  // floor ~= O(sqrt(R)) keeps "some per-role coverage" without exploding work.
  const r = roles_len > 0 ? roles_len : 1;
  const by_role = p.pool_per_role_min * Math.ceil(Math.sqrt(r));
  return by_role > p.pool_abs_min ? by_role : p.pool_abs_min;
}

function pool_limit(n: number, roles_len: number, mix: number, k: number, p: RoleSelectPolicy): number {
  const kk = k > 1 ? k : 1;
  const c = p.pool_sqrt_k_c_min + (p.pool_sqrt_k_c_max - p.pool_sqrt_k_c_min) * (1 - mix);
  const base = Math.ceil(k + c * Math.sqrt(kk));
  const floor = pool_floor(roles_len, p);
  const lim = base > floor ? base : floor;
  return lim < n ? lim : n;
}

// FULL: collect ALL unique candidates in selector scope.
function resolve_role_full(role_index: RoleIndex, roles: number[]): PID[] {
  const seen = new Set<PID>();
  const out: PID[] = [];

  for (const r of roles) {
    for (const pid of role_index.lut[r].iter()) {
      if (seen.has(pid)) continue;
      seen.add(pid);
      out.push(pid);
    }
  }
  return out;
}

// PARTIAL: build a bounded candidate pool (deduped, role-hedged).
function resolve_role_partial(role_index: RoleIndex, roles: number[], seed: bigint, pool_lim: number): PID[] {
  if (pool_lim <= 0) return [];

  // deterministic per-call RNG (seed 고정이면 동일 결과)
  const rng = make_rng(seed ^ 0x243f6a8885a308d3n);

  // role mass 기반으로 cap 배분
  let total = 0;
  for (const r of roles) total += role_index.role_mass(r);
  if (total <= 0) total = roles.length;

  const seen = new Set<PID>();
  const out: PID[] = [];

  type Slot = { r: number; it: IterableIterator<PID> };

  // role별 “기대 비율”과 “실제 진행률”로 cap를 부드럽게 조절
  const exp = new Map<number, number>(); // expected share (0..1)
  const got = new Map<number, number>(); // yielded count per role
  let out_total = 0; // total yielded (all roles)

  for (let i = 0; i < roles.length; i++) {
    const r = roles[i];
    const m = role_index.role_mass(r);
    exp.set(r, total > 0 ? m / total : 1 / roles.length);
    got.set(r, 0);
  }

  const cap_for = (r: number): number => {
    const e = exp.get(r) ?? 1 / roles.length;
    const g = got.get(r) ?? 0;
    // deficit = 기대치 - 실제치 (음수면 0으로 클램프)
    const deficit = Math.max(0, e * out_total - g);
    // base는 over*expected, deficit이 커질수록 cap 증가(연속성)
    const base = 3 * e; // INTERNAL: role window size multiplier (old "oversample" 역할)
    // cap 최소 1. deficit 스케일은 “현재까지 out_total”에 자연히 비례.
    return 1 + Math.ceil(base * (1 + deficit));
  };

  const make_slot = (r: number): Slot | null => {
    const set = role_index.lut[r];
    const sz = set.size();
    if (sz <= 0) return null;
    const cap = cap_for(r); // <-- 진행률 기반 cap

    const start = set.kth((rng()) % sz);
    if (start === undefined) return null;
    // 역할 내부는 "원형 창"에서 그냥 뽑는다 (전역 rank-mix는 OUTSIDE)
    return { r, it: set.iter_from(start, cap) };
  };

  const slots: Slot[] = [];
  for (let i = 0; i < roles.length; i++) {
    const s = make_slot(roles[i]);
    if (s) slots.push(s);
  }
  if (!slots.length) return [];

  // round-robin until stalled; if stalled, STOP (caller may re-call with new seed)
  for (;;) {
    if (out.length >= pool_lim) break;
    let progressed = 0;
    for (let i = 0; i < slots.length; i++) {
      if (out.length >= pool_lim) break;
      const s = slots[i];
      const n = s.it.next();
      if (n.done) continue;
      const pid = n.value;
      if (seen.has(pid)) continue;
      seen.add(pid);
      progressed++;
      out_total++;
      got.set(s.r, (got.get(s.r) ?? 0) + 1);
      out.push(pid);
    }
    if (!progressed) break;
  }
  return out;
}

export function resolve_roles(
  role_index: RoleIndex,
  roles: number[],
  n: number,
  seed: bigint,
  mix: number,
  fanout: number,
  score_of: ScoreFn,
  policy: RoleSelectPolicy = DEFAULT_POLICY,
): PID[] {
  if (choose_full(n, fanout, policy)) {
    const pool = resolve_role_full(role_index, roles);
    return rankmix_full(pool, seed, mix, score_of);
  }

  const pool_lim = pool_limit(n, roles.length, mix, fanout, policy);
  const pool = resolve_role_partial(role_index, roles, seed, pool_lim);
  return rankmix_partial(pool, seed, mix, score_of, fanout);
}

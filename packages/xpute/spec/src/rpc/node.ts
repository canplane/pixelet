// @xpute/spec/rpc/node.ts

import { BIT64, HAS_BITS64, type u32, type u64 } from "@xpute/spec/lib/kit/word.ts";

/**
 * NodeSelector (u64 as bigint)
 *
 * Layout:
 *   bit63: MODE
 *     0 = ID-mode   (concrete node id)
 *     1 = ROLE-mode (role mask selector)
 *
 * [ID-mode]
 *   bits 0..62: nodeId
 *     0 = LOOPBACK
 *     1..(2^63-2) = node id
 *
 * [ROLE-mode]
 *   bits 0..62: roleMask
 *     default semantics: ANYCAST (fanout policy decides actual delivery)
 *
 * Special values:
 *   LOOPBACK  = 0
 *   UNIVERSAL = all-ones (bit63=1, roleMask=all-ones)
 *
 * Notes:
 * - ROLE-mode does not target a specific node. It is a selector.
 * - fanout semantics are handled by NIC / routing policy.
 */

/**
 * NodeSelector: Routing selector (in frame.sel)
 * - ID-mode: route to specific node
 * - ROLE-mode: route to role(s) - anycast/multicast
 * - Opaque to dispatch/device code, interpreted by runtime/gateway
 */
export type NodeSelector = u64;

export const MODE_BIT: u32 = 63;
export const MODE_MASK: u64 = BIT64(MODE_BIT);
export const VALUE_MASK: u64 = MODE_MASK - 1n; // low 63 bits

/** ID-mode: local loopback */
export const LOOPBACK: NodeSelector = 0n;

/** ROLE-mode: global selector (all remote nodes) */
export const UNIVERSAL: NodeSelector = MODE_MASK | VALUE_MASK;

/** ROLE bit (n must be 0..62) */
const ROLE_SEL = (n: u32): NodeSelector => MODE_MASK | BIT64(n); // caller guarantees 0..62

/**
 * ROLE bits (capabilities / identity flags)
 * combinable bitmask
 */
export const Role = {
  SERVER: ROLE_SEL(0), // 서비스를 제공하는 백엔드/인프라 노드
  CLIENT: ROLE_SEL(1), // 서비스를 소비하는 브라우저/앱 노드
  GUEST: ROLE_SEL(2), // 인증되지 않았거나 제한된 권한의 노드
  BOT: ROLE_SEL(3), // 자동화된 에이전트 또는 더미 노드
} as const;

export const Node = { ...Role, LOOPBACK, UNIVERSAL } as const;

/**
 * Extractors / predicates
 *
 * VALUE(sel):
 *   ID-mode   => nodeId
 *   ROLE-mode => roleMask
 */
export const VALUE = (sel: NodeSelector): u64 => sel & VALUE_MASK;

export const IS_ROLE_SEL = (sel: NodeSelector): boolean => HAS_BITS64(sel, MODE_MASK);
export const IS_ID_SEL = (sel: NodeSelector): boolean => !IS_ROLE_SEL(sel);

export const IS_LOOPBACK = (sel: NodeSelector): boolean => sel === LOOPBACK;
export const IS_UNIVERSAL = (sel: NodeSelector): boolean => sel === UNIVERSAL;

/**
 * Get role bit indices from ROLE-mode selector.
 * Returns array of set bit positions (0..62).
 *
 * NOTE:
 * - mode bit (bit63) is not part of role mask.
 * - loop is 0..62 only.
 *
 * Precondition: IS_ROLE(sel) is true.
 */
export const role_indices = (sel: NodeSelector): u32[] => {
  // Precondition: IS_ROLE_SEL(sel) === true
  const mask = VALUE(sel); // low 63-bit roleMask
  const ret: u32[] = [];
  for (let i = 0; i < MODE_BIT; i++) {
    if (mask & BIT64(i)) ret.push(Number(i));
  }
  return ret;
};

/**
 * fanout hint (interpreted by NIC)
 */
export enum FANOUT {
  AUTO = 0, // NIC decides / clamps
  ONE = 1, // send once
  ALL = 0xffffffff, // as many as possible (NIC clamps)
}

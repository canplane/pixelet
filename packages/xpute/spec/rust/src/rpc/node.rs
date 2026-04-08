// @xpute/spec/rpc/node.rs

use crate::lib::kit::word::{bit64, has_bits64};

/// NodeSelector: Routing selector (in frame.sel)
/// - ID-mode: route to specific node
/// - ROLE-mode: route to role(s) - anycast/multicast
/// - Opaque to dispatch/device code, interpreted by runtime/gateway
///
/// Layout:
///   bit63: MODE
///     0 = ID-mode   (concrete node id)
///     1 = ROLE-mode (role mask selector)
///
/// [ID-mode]
///   bits 0..62: nodeId
///     0 = LOOPBACK
///     1..(2^63-2) = node id
///
/// [ROLE-mode]
///   bits 0..62: roleMask
///     default semantics: ANYCAST (fanout policy decides actual delivery)
///
/// Special values:
///   LOOPBACK  = 0
///   UNIVERSAL = all-ones (bit63=1, roleMask=all-ones)
///
/// Notes:
/// - ROLE-mode does not target a specific node. It is a selector.
/// - fanout semantics are handled by NIC / routing policy.
pub type NodeSelector = u64;

pub const MODE_BIT: u32 = 63;
pub const MODE_MASK: u64 = bit64(MODE_BIT);
pub const VALUE_MASK: u64 = MODE_MASK - 1; // low 63 bits

/// ID-mode: local loopback
pub const LOOPBACK: NodeSelector = 0;

/// ROLE-mode: global selector (all remote nodes)
pub const UNIVERSAL: NodeSelector = MODE_MASK | VALUE_MASK;

const fn role_sel(n: u32) -> NodeSelector { MODE_MASK | bit64(n) } // caller guarantees 0..62

/// ROLE bits (capabilities / identity flags)
/// combinable bitmask
pub struct Role;
impl Role {
    pub const SERVER: NodeSelector = role_sel(0); // 서비스를 제공하는 백엔드/인프라 노드
    pub const CLIENT: NodeSelector = role_sel(1); // 서비스를 소비하는 브라우저/앱 노드
    pub const GUEST: NodeSelector = role_sel(2);  // 인증되지 않았거나 제한된 권한의 노드
    pub const BOT: NodeSelector = role_sel(3);    // 자동화된 에이전트 또는 더미 노드
}

pub struct Node;
impl Node {
    pub const SERVER: NodeSelector = Role::SERVER;
    pub const CLIENT: NodeSelector = Role::CLIENT;
    pub const GUEST: NodeSelector = Role::GUEST;
    pub const BOT: NodeSelector = Role::BOT;
    pub const LOOPBACK: NodeSelector = LOOPBACK;
    pub const UNIVERSAL: NodeSelector = UNIVERSAL;
}

// ============ Extractors / predicates ============

/// VALUE(sel):
///   ID-mode   => nodeId
///   ROLE-mode => roleMask
pub fn value(sel: NodeSelector) -> u64 { sel & VALUE_MASK }

pub fn is_role_sel(sel: NodeSelector) -> bool { has_bits64(sel, MODE_MASK) }
pub fn is_id_sel(sel: NodeSelector) -> bool { !is_role_sel(sel) }

pub fn is_loopback(sel: NodeSelector) -> bool { sel == LOOPBACK }
pub fn is_universal(sel: NodeSelector) -> bool { sel == UNIVERSAL }

/// Get role bit indices from ROLE-mode selector.
/// Returns array of set bit positions (0..62).
///
/// NOTE:
/// - mode bit (bit63) is not part of role mask.
/// - loop is 0..62 only.
///
/// Precondition: IS_ROLE(sel) is true.
pub fn role_indices(sel: NodeSelector) -> Vec<u32> {
    // Precondition: is_role_sel(sel) === true
    let mask = value(sel); // low 63-bit roleMask
    let mut ret = Vec::new();
    for i in 0..MODE_BIT {
        if has_bits64(mask, bit64(i)) {
            ret.push(i);
        }
    }
    ret
}

/// fanout hint (interpreted by NIC)
pub struct Fanout;
impl Fanout {
    pub const AUTO: u32 = 0;         // NIC decides / clamps
    pub const ONE: u32 = 1;          // send once
    pub const ALL: u32 = 0xffffffff; // as many as possible (NIC clamps)
}

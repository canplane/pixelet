// @xpute/spec/net/xport/mod.ts

import type { Bytes } from "@xpute/spec/lib/kit/array.ts";
import { Errno } from "@xpute/spec/sys/errno.ts";

import { Vector } from "@xpute/spec/control/vec.ts";
import type { NodeSelector } from "@xpute/spec/rpc/node.ts";
import type { RID } from "@xpute/spec/rpc/rid.ts";
import type { IdentityKey, PID, Signature } from "../crypto.ts";

import type { CentralAdvert } from "./central.ts";
import type { MeshAdvert } from "./mesh.ts";

export type Key = string; // opaque key (rid 등)

// Opaque transport link handle. String shape is transport-owned. Gateway/Hello must not parse.
export type Handle = string;

// Dial: transport-consumed bootstrap hint (string ABI).
// - Not overlay identity (PID)
// - Not transport link handle (Handle)
// - Used only to create/seed an initial link; HELLO later binds PID <-> Handle.
// - Format is transport-defined (often URL-like in CENTRAL, but not required to be URL/URI).
export type Dial = string; // e.g. "https://...", "wss://...", "turn:...", "rtc:..."

/**
 * ========= Network naming canon (NO "locator") =========
 *
 * PID:
 * - Overlay identity (stable per node).
 * - Used for crypto identity (keys) and overlay routing selection.
 *
 * Handle:
 * - Transport-owned link instance identifier (NOT identity).
 * - Opaque link instance id chosen by the transport.
 * - May change across reconnects/failovers (transport policy), but can also remain stable
 *   while underlying socket/session ids churn.
 * - Examples:
 *   - ws: transport-owned link id (socket-backed internally)
 *   - webrtc: data-channel id / (fingerprint,label) / ice-path key
 *
 * Dial:
 * - Bootstrap-only dial hint (manifest seed).
 * - Used to create an initial link; HELLO later binds PID <-> handle internally.
 *
 * Forbidden:
 * - "locator" is banned (ambiguous: Dial vs Handle vs PID). Use Dial or Handle explicitly.
 *
 *  ========= Trust & authentication contract =========
 *
 *  - PID is NOT trusted until crypto verifies it:
 *    A transport-reported inbound link (src_handle) is only a transport hint until:
 *      (a) HELLO completes, and
 *      (b) subsequent packets pass signature verification against registered keys.
 *
 * - Pre-auth exception is HELLO only:
 *    transports MAY accept/route NET_HELLO with plaintext enc_hdr under allowlist,
 *    but MUST treat all other vectors as authenticated traffic (drop if unknown handle/peer).
 *
 *  ========= CENTRAL: bootstrap -> post-HELLO (Handle <-> PID bind) =========
 *
 *  Goal:
 *  - Start with Dial seeds (bootstrap) and end with verified PID-bound routing (Handle <-> PID).
 *
 *  Terms (CENTRAL):
 *  - Dial: manifest seed string (server/base URL). Bootstrap-only.
 *  - Link handle (ws): transport-owned link id (implementation-defined; often socket-backed).
 *
 *  Phases:
 *
 *  1) BOOTSTRAP (pre-HELLO)
 *    - Transport creates a link (handle) from Dial seed.
 *    - All handshake state is keyed by Handle.
 *
 *  2) HANDSHAKE (HELLO in flight)
 *    - Retry/ACK-stop state is keyed by Handle (not PID).
 *    - Cookie binding is bound to Handle.
 *
 *  3) POST-HELLO
 *    - After HELLO verifies identity, transport binds Handle <-> PID.
 *    - After bind, transport routes by PID internally.
 *
 *  Termination rule:
 *  - After HELLO bind completes, outbound routing must not use Dial.
 */

// ============ Transport ============

export const enum TransportKind {
  CENTRAL = "central",
  MESH = "mesh",
}

// ============ Handle helpers (transport-owned opaque id) ============
// Shared helpers for handle allocation/validation by transport kind.
// Keep shape policy centralized to avoid per-plug drift.
export const handle_prefix = (kind: TransportKind): string => `${kind}:`;

export const MAX_HANDLE_LEN = 256;
export const is_handle = (s: unknown): s is Handle => typeof s === "string" && s.length > 0 && s.length <= MAX_HANDLE_LEN;

export const is_handle_of = (kind: TransportKind, s: unknown): s is Handle => is_handle(s) && s.startsWith(handle_prefix(kind));

export const alloc_handle = (kind: TransportKind): Handle => `${handle_prefix(kind)}${globalThis.crypto.randomUUID()}`;

// gateway가 transport로부터 받는 inbound 이벤트.
// - src_handle: transport link handle (ws: transport-owned handle / webrtc: dc-id 등)
// callback is core-owned/internal. If it throws, implementation MAY let it propagate (fatal policy).
export type RxFn = (src_handle: Handle, rid: RID, enc_hdr: Bytes, enc_body: Bytes | null, sig: Signature) => void;

// transport -> gateway (liveness / teardown / membership hints)
//
// Contract notes (common to CENTRAL / MESH):
// - Events are transport policy outputs (not raw plug/socket callbacks).
// - Implementations may suppress/translate/dedup lower-level events.
// - Late/stale events are allowed internally; transports should guard stale handles / stale pid epochs.
// - PEER_UNREACHABLE is a terminal PID-scoped policy decision for the CURRENT live binding epoch.
export type TransportEvent =
  | { vec: Vector.LINK_UP; handle: Handle; pid?: PID; errno?: Errno }
  | { vec: Vector.LINK_DOWN; handle: Handle; pid?: PID; reason?: string; errno?: Errno }
  | { vec: Vector.PEER_UNSTABLE; handle: Handle; pid?: PID; reason?: string; errno?: Errno }
  // terminal peer-level decision (pid-scoped)
  | { vec: Vector.PEER_UNREACHABLE; pid: PID; reason?: string; errno?: Errno };

// callback is core-owned/internal. If it throws, implementation MAY let it propagate (fatal policy).
export type TransportEventFn = (ev: TransportEvent) => void;

export type Transport = {
  readonly kind: TransportKind;

  // pre-auth HELLO allowlist gate (transport-local wire sniff)
  is_pre_auth_allowed(enc_hdr: Bytes, has_body: 0 | 1): boolean;

  // Dial bootstrap (transport-owned)
  // examples: ["http://", "https://", "ws://", "wss://"], ["turn:", "rtc:"]
  dial_prefixes(): readonly string[];
  /** @throws NetworkFaultError */
  dial_seed(dial: Dial): Promise<Handle>;

  // dst는 Peer로만 받는다: "어떤 xport인지"는 peer.kind로 이미 정해짐.
  // Dial/Addr 같은 라우팅 키는 peer에 넣지 않는다 (lane 선택/매핑은 transport 내부 규칙).
  /** @throws NetworkFaultError */
  send(dst: Peer, rid: RID, enc_hdr: Bytes, enc_body: Bytes | null, sig: Signature): void;
  // HELLO / pre-auth path: handle-addressed send
  /** @throws NetworkFaultError */
  send_hello(dst_handle: Handle, rid: RID, enc_hdr: Bytes, sig: Signature): void;

  // inbound: transport always reports src_handle (link handle).
  // hello/retransmit/bind state machines are keyed by Handle.
  on(rx: RxFn): void;

  /**
   * transport liveness/teardown events (REQUIRED)
   * - LINK_UP / LINK_DOWN:
   *   transport link edge hints (handle-scoped; reconnect/connectivity transitions)
   * - PEER_UNSTABLE:
   *   peer-level degradation hint (handle-scoped observation, no unregister implied)
   * - PEER_UNREACHABLE:
   *   transport policy declares PID unreachable/stale (after grace/ttl etc.)
   *
   * NOTE: These are routing/membership hints. NetGateway decides how to map them
   * into PeerDirectory.unregister() and/or host-visible signals.
   *
   * Canonicalization policy:
   * - Implementations MAY translate/suppress plug-native events to preserve stable external semantics.
   * - Duplicate terminal decisions for the same PID/live-binding epoch SHOULD be suppressed.
   * - Late events after teardown/rebind MAY be dropped silently.
   * - LINK_UP semantics may differ by transport policy:
   *   - CENTRAL may expose reconnect/physical link-up hints on stable handles
   *   - MESH may reserve canonical LINK_UP for HELLO-authenticated bind completion
   *   Consumers should treat LINK_UP as a transport hint and apply kind-aware policy if needed.
   *
   * Contract:
   * - Implementations that have nothing to report MUST still accept the handler (noop).
   */
  on_event(ev: TransportEventFn): void;

  /**
   * Bind a verified overlay PID to a transport handle.
   * - Called exactly when HELLO verifies identity.
   * - Transport uses this for pid->handle routing and for handle->pid hints on handle_down.
   *
   * Contract:
   * - Must be accepted by all transports.
   * - No plug-level key migration is implied (NO rekey).
   *
   * Epoch note:
   * - A successful bind_handle_pid() starts/refreshes the current live binding epoch for that PID.
   * - Transports that latch terminal PEER_UNREACHABLE per PID SHOULD clear that latch here.
   */
  bind_handle_pid(handle: Handle, pid: PID): void;
  /**
   * ========= Implementation note (WebRTC) =========
   *
   * MESH/RTC-style transports often have:
   * - pid <-> handle 1:N (multiple active links per peer)
   * - handle churn (ICE restart, reconnection, NAT rebinding)
   *
   * Recommendation (implementation-internal only):
   * - keep HandleMultipathMaps (pid -> ordered handles[], handle -> pid)
   * - define "primary handle" rule (handles[0]) and bounded failover attempts per send()
   * - do NOT leak handle selection into NetGateway/Hello APIs (keep interface stable)
   */
};

// ============ Central/Mesh specialized transports ============

export type CentralTransport = Transport & {
  readonly kind: TransportKind.CENTRAL;
  bind_handle_advert(handle: Handle, advert?: CentralAdvert): void;
};
export type MeshTransport = Transport & {
  readonly kind: TransportKind.MESH;
  bind_handle_advert(handle: Handle, advert?: MeshAdvert): void;
};

// Thin typed registry (key->value correlation preserved).
export type Transports = {
  [TransportKind.CENTRAL]: CentralTransport;
  [TransportKind.MESH]: MeshTransport;
};

// /**
//  * ========= Wire/Store note (WebRTC) =========
//  *
//  * A WebRTC transport MAY be wire-only:
//  * - If the data-channel is reliable/ordered and payload sizes are acceptable,
//  *   body can be sent inline without Store.
//  *
//  * A WebRTC transport MAY also adopt Store:
//  * - For large blobs, resumability, relay fallback, or outbox semantics.
//  *
//  * Keeping Wire and Store as separate primitives preserves both options.
//  */

// // ========= WebRTC multipath (proposal) =========
// // Transport 구현체 내부에서 pid당 여러 handle을 허용할 때 권장 구조.
// // (인터페이스 변경 없이 구현체 내부에서만 사용)
// export type HandleMultipathMaps = {
//   // pid -> ordered handles (0번이 primary)
//   handles_by_pid: Map<PID, Handle[]>;
//   // handle -> pid (reverse)
//   pid_by_handle: Map<Handle, PID>;
// };

// ============ Peer ============

/**
 * Peer: Remote node in overlay network
 * - Learned via HELLO handshake
 * - Contains remote public keys
 * - Used for routing + encryption
 */
type PeerBase = {
  // ---- core ----
  pid: PID;
  role_mask: NodeSelector;

  sign_pub: IdentityKey; // ed25519 pub (32)
  dh_pub: IdentityKey; // x25519 pub (32)

  // ---- meta ----
  kind: TransportKind;
  // last_seen?: number; // ms (optional)
};

export type CentralPeer = PeerBase & {
  kind: TransportKind.CENTRAL;
  // ---- remote -> local advert hints (optional) ----
  // transport policy metadata learned via HELLO (CENTRAL hints only).
  advert?: CentralAdvert;
};
export type MeshPeer = PeerBase & {
  kind: TransportKind.MESH;
  // ---- remote -> local advert hints (optional) ----
  // transport policy metadata learned via HELLO (MESH hints only).
  advert?: MeshAdvert;
};

export type Peer = CentralPeer | MeshPeer;

export type PeerByKind = {
  [TransportKind.CENTRAL]: CentralPeer;
  [TransportKind.MESH]: MeshPeer;
};

export type Advert = CentralAdvert | MeshAdvert;

export type AdvertByKind = {
  [TransportKind.CENTRAL]?: CentralAdvert;
  [TransportKind.MESH]?: MeshAdvert; // WebRTC/Mesh runtime hints (browser-only consumer; server can ignore)
};

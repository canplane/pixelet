// @xpute/spec/control/vec.ts

// u16
const NET_MASK = 0xff00;
export const enum Vector {
  // NOTE: numbering is not final; OK to reshuffle later.

  // 🔴 Critical Zone (0x00 ~ 0x0F) - Kernel Native
  BOOT = 0x0000,
  SYNC = 0x0001, // Time Synchronization
  PANIC = 0x0002, // System Halt

  // 🟡 System Zone (0x10 ~ 0x1F) - Overridable Middleware
  // 엔진이 커널에게 위임(Delegate)받는 영역
  AUTH = 0x0010, // Login / Session Verification
  TILE = 0x0011, // Voxel Chunk Request (Spatial Query)

  // 🟢 User Zone (0x20 ~ 0xFF) - Pure User Interaction

  // ---- infra / transport control (kernel-ish) ----
  // Not user payload vectors. Used for internal control plane signalling.
  NET_HELLO = NET_MASK | 0x0001,

  LINK_UP = NET_MASK | 0x0002,
  LINK_DOWN = NET_MASK | 0x0003,
  PEER_UNSTABLE = NET_MASK | 0x0004,
  PEER_UNREACHABLE = NET_MASK | 0x0005,

  // Mesh/WebRTC signaling
  RTC_SIGNAL_JOIN = NET_MASK | 0x0006,
  RTC_SIGNAL_PEER_JOINED = NET_MASK | 0x0007,
  RTC_SIGNAL_PEER_LEFT = NET_MASK | 0x0008,
  RTC_SIGNAL_OFFER = NET_MASK | 0x0009,
  RTC_SIGNAL_ANSWER = NET_MASK | 0x000a,
  RTC_SIGNAL_ICE = NET_MASK | 0x000b,
  RTC_SIGNAL_RENEGOTIATE = NET_MASK | 0x000c,
}

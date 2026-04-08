// @xpute/spec/control/vec.rs

// u16
const NET_MASK: u16 = 0xff00;

pub struct Vector;
impl Vector {
    // NOTE: numbering is not final; OK to reshuffle later.

    // 🔴 Critical Zone (0x00 ~ 0x0F) - Kernel Native
    pub const BOOT: u16 = 0x0000;
    pub const SYNC: u16 = 0x0001; // Time Synchronization
    pub const PANIC: u16 = 0x0002; // System Halt

    // 🟡 System Zone (0x10 ~ 0x1F) - Overridable Middleware
    // 엔진이 커널에게 위임(Delegate)받는 영역
    pub const AUTH: u16 = 0x0010; // Login / Session Verification
    pub const TILE: u16 = 0x0011; // Voxel Chunk Request (Spatial Query)

    // 🟢 User Zone (0x20 ~ 0xFF) - Pure User Interaction

    // ---- infra / transport control (kernel-ish) ----
    // Not user payload vectors. Used for internal control plane signalling.
    pub const NET_HELLO: u16 = NET_MASK | 0x0001;

    pub const LINK_UP: u16 = NET_MASK | 0x0002;
    pub const LINK_DOWN: u16 = NET_MASK | 0x0003;
    pub const PEER_UNSTABLE: u16 = NET_MASK | 0x0004;
    pub const PEER_UNREACHABLE: u16 = NET_MASK | 0x0005;

    // Mesh/WebRTC signaling
    pub const RTC_SIGNAL_JOIN: u16 = NET_MASK | 0x0006;
    pub const RTC_SIGNAL_PEER_JOINED: u16 = NET_MASK | 0x0007;
    pub const RTC_SIGNAL_PEER_LEFT: u16 = NET_MASK | 0x0008;
    pub const RTC_SIGNAL_OFFER: u16 = NET_MASK | 0x0009;
    pub const RTC_SIGNAL_ANSWER: u16 = NET_MASK | 0x000a;
    pub const RTC_SIGNAL_ICE: u16 = NET_MASK | 0x000b;
    pub const RTC_SIGNAL_RENEGOTIATE: u16 = NET_MASK | 0x000c;
}

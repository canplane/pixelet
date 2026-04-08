// @xpute/spec/ipc/frame.rs

//! Frame Hierarchy
//!
//! Frame
//! ├─ ControlFrame (WorkerInit | LinkInitSyn | LinkInitAck)
//! └─ DataFrame (SignalFrame | PostFrame)

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::control::cmd::Command;
use crate::ipc::topology::{DevicePort, WorkerId};
use crate::lib::kit::array::U8Array;
use crate::rpc::node::NodeSelector;
use crate::rpc::rid::RID;
use crate::sys::errno::Errno;

pub type Vector = u16;

/// Frame kind discriminator
/// - Control plane: 0x80+ (worker/link management)
/// - Data plane: 0x01-0x7F (application payloads)
const CTRL_MASK: u8 = 0x80;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum FrameKind {
    // Control Plane
    WorkerReady = CTRL_MASK, // or next free
    WorkerInit = CTRL_MASK | 0x01,
    LinkInitSyn = CTRL_MASK | 0x02,
    LinkInitAck = CTRL_MASK | 0x03,

    // Data Plane
    Signal = 0x01,
    Post = 0x02,
}

/// Single unified link for all frame kinds.
/// Control and data frames share the same transport channel.
pub trait WorkerLink {
    fn send(&self, frame: Frame);
    fn on(&self, handler: Box<dyn Fn(Frame)>) -> Box<dyn FnOnce()>;
}

pub type WorkerAttr = ();

// ============ Control Plane ============

#[derive(Clone, Debug)]
pub struct WorkerReady {
    pub kind: FrameKind,
}

#[derive(Clone, Debug)]
pub struct WorkerInit {
    pub kind: FrameKind,
    pub worker_id: WorkerId,
}

#[derive(Clone)]
pub struct WorkerArg {
    pub worker_id: WorkerId,
    pub uplink: std::rc::Rc<dyn WorkerLink>,
}

/// Link Initialization Protocol
/// Handshake between host and worker to establish routing tables
#[derive(Clone, Debug)]
pub struct LinkInitSyn {
    pub kind: FrameKind,

    pub port: DevicePort,
    pub capable_vecs: Vec<Vector>,  // Capability vectors this worker handles
    pub capable_cmds: Vec<Command>, // Commands this worker handles
}

#[derive(Clone, Debug)]
pub struct LinkInitAck {
    pub kind: FrameKind,

    pub vec_table: Vec<(Vector, DevicePort)>, // vec -> port mapping
    pub cmd_table: Vec<(Command, DevicePort)>, // cmd -> port mapping
}

#[derive(Clone, Debug)]
pub enum ControlFrame {
    WorkerReady(WorkerReady),
    WorkerInit(WorkerInit),
    LinkInitSyn(LinkInitSyn),
    LinkInitAck(LinkInitAck),
}

// ============ Transaction Options (layer-specific hints) ============

// Local transfer hint
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TxOptions {
    // tx controller hint
    pub timeout: Option<u32>, // Hard deadline: 이 시간이 지나면 큐에 남은 데이터도 모두 폐기됩니다.

    // network gateway hint
    pub fanout: Option<u32>, // Multicast count
}

/// Operation Flags (transport-layer control bits)
///
/// NOTE: Consider using the `bitflags` crate for ergonomic flag operations
/// (contains, insert, remove, etc.) when this module grows.
pub mod frame_flag {
    // RES: response bit (REQ is implicit when bit=0)
    pub const RES: u32 = 0x01;

    // Reliability
    pub const ACKREQ: u32 = 0x10; // Require acknowledgement (reliable delivery)

    // Network hints (gateway / transport interprets these)
    //
    // CACHE is a RID-addressable fetch hint.
    //
    // It does NOT mean "always use HTTP", and it does NOT replace the normal
    // WS/mesh signaling path. The signaling/control frame still flows normally;
    // CACHE only adds an optional fetch-style consumption semantic around RID.
    //
    // Canonical meaning:
    // - The payload/resource identified by (rid) may be treated as a stable,
    //   fetchable object by transports that support split wire/store delivery
    //   (e.g. CENTRAL ws + http outbox).
    // - Transport MAY ignore this bit if the transport has no cache/fetch plane
    //   (e.g. current MESH/WebRTC path).
    //
    // Directional policy:
    //
    // 1) client -> server
    //    a) body == null:
    //       - CACHE is meaningful.
    //       - The request is a deterministic RID-addressable lookup.
    //       - WS signaling still carries the request frame.
    //       - A transport MAY additionally use/prefetch/serve an HTTP GET-style
    //         path keyed by rid, if its policy supports that.
    //
    //    b) body != null:
    //       - CACHE is ignored.
    //       - Body-bearing requests are not treated as cacheable fetch objects.
    //
    // 2) server -> client
    //    a) body == null:
    //       - CACHE is nonsensical / no-op.
    //       - There is nothing to fetch, so no RID fetch semantic is created.
    //
    //    b) body != null:
    //       - CACHE is meaningful.
    //       - The sender MAY stage the payload into an outbox/store under rid,
    //         and use WS only as the notify/control path.
    //       - The receiver may then consume the payload by RID-based GET/pull,
    //         instead of relying on inline body delivery semantics.
    //
    // Practical rule:
    // - CACHE is only meaningful when the payload shape and direction make RID-
    //   based fetch semantics possible.
    // - If not, transports should treat CACHE as a hint and safely ignore it.
    //
    // Current expected transport behavior:
    // - CENTRAL:
    //   understands CACHE as a split wire/store hint around rid.
    // - MESH:
    //   currently ignores CACHE.
    pub const CACHE: u32 = 0x20;
}

// ============ Data Plane ============

// NOTE: TS uses intersection type (`DataFrameBase & { ... }`) to compose frame types.
// In Rust, fields are flattened directly into each frame struct — no nesting needed.
// `DataFrameBase` is kept as a standalone doc struct for reference only.
#[derive(Clone, Debug)]
pub struct DataFrameBase {
    pub tx_id: u32,            // transaction identifier (correlates request/response)
    pub errno: Errno,          // OK=0, error<0 (negative errno)
    pub flags: u32,            // FrameFlag bits
    pub opts: TxOptions,       // local-only routing/tx hints; not on wire header
    pub args: U8Array,         // TLV stream (codec-defined terminator)
    pub body: Option<U8Array>, // raw; TODO: body will be replaced with SAB pointer handle
}

/// Frame: Message envelope shared across dispatch, runtime, and network boundaries
/// - sel: routing selector (where to send)
/// - vec/cmd: handler selector (what to do)
/// - tx_id: transaction tracking
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignalFrame {
    pub tx_id: u32,
    pub errno: Errno,
    pub flags: u32,
    pub opts: TxOptions,
    pub args: U8Array,
    pub body: Option<U8Array>, // TODO: replace with SAB pointer handle

    pub kind: FrameKind,
    pub sel: NodeSelector,
    // Correspondent selector (routing metadata).
    //
    // Directional semantics:
    // - REQ (RES=0): selector of the intended correspondent (destination on the wire).
    // - RES (RES=1): selector of the intended correspondent (original requester on the wire).
    //
    // Contract:
    // - Router/gateway may READ sel for routing, but MUST NOT rewrite it.
    // - Caller identity is NOT inferred from sel. If needed, encode it elsewhere (e.g. RID/args/body).
    pub rid: Option<RID>,
    pub vec: Vector,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PostFrame {
    pub tx_id: u32,
    pub errno: Errno,
    pub flags: u32,
    pub opts: TxOptions,
    pub args: U8Array,
    pub body: Option<U8Array>, // TODO: replace with SAB pointer handle

    pub kind: FrameKind,
    pub cmd: Command,
}

#[derive(Clone, Debug)]
pub enum DataFrame {
    Signal(SignalFrame),
    Post(PostFrame),
}

/// Top-level Frame union
/// All messages that flow through the bus
#[derive(Clone, Debug)]
pub enum Frame {
    Control(ControlFrame),
    Data(DataFrame),
}

// Used for transport-level NACK/fault replies and for omitted payloads.
pub fn empty_args() -> U8Array {
    Vec::new()
}

// @xpute/spec/ipc/device.ts

use std::rc::Rc;

use futures::future::LocalBoxFuture;

use crate::control::cmd::Command;
use crate::control::post::CommandPort;
use crate::control::signal::VectorPort;
use crate::ipc::frame::{DataFrame, SignalFrame, Vector};

/// App-side Module Execution Environment
pub struct XputeContext {
    pub post: CommandPort,
}

/// Device-side Module Execution Environment
#[derive(Clone)]
pub struct DeviceEnvironment {
    pub signal: VectorPort,
    pub post: CommandPort,
}

/// DeviceUplink — unified data-plane entry point (host <-> worker/device <-> network)
///
/// Scope:
/// - `DeviceUplink` is the transport surface used by dispatchers and devices for DataFrame (SIGNAL/POST).
/// - Dispatchers and devices must treat routing/reliability/directionality as opaque.
///
/// Selector (sel) contract (SIGNAL only):
/// - `sel` is transported and delivered verbatim (no rewriting).
/// - Its meaning is transport-defined; dispatchers and devices treat it as opaque metadata.
///
/// FrameFlag.RES ownership:
/// - Dispatch layers and devices MUST NOT set/clear FrameFlag.RES.
/// - The runtime/router owns RES for request/response directionality.
///
/// Reliability (ACKREQ):
/// - ACKREQ=0 (UNACK): runtime MUST NOT produce follow-up frames for this tx_id.
/// - ACKREQ=1 (ACKREQ): runtime MUST produce at least one follow-up frame (may be args=EMPTY_ARGS, body=null).
///
/// wait() contract:
/// - wait(tx_key) yields the next inbound DataFrame for that tx, or null on timeout EOF.
/// - Type of returned frame (SIGNAL/POST) is determined by runtime routing.
pub trait DeviceUplink {
    fn send(&self, frame: DataFrame) -> i32; // enqueue/emit a data-plane frame
    fn wait(&self, tx_key: i32) -> LocalBoxFuture<'static, Option<DataFrame>>; // wait for next ACK frame of tx_key (or null on EOF)
}

pub trait NetUplink {
    fn send(&self, frame: SignalFrame);
}

/// [Device]
/// 시스템 버스에 매핑된 추상화된 기능 단위입니다.
/// 모든 장치는 이 규격을 통해 신호를 수신하는 '수동적 종착점'이 됩니다.
pub trait Device {
    fn capable_vecs(&self) -> Vec<Vector>;
    fn capable_cmds(&self) -> Vec<Command>;

    fn receive(&self, frame: DataFrame) -> LocalBoxFuture<'static, DataFrame>;
}

pub type DeviceUplinkRef = Rc<dyn DeviceUplink>;

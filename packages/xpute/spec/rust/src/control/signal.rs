// @xpute/spec/control/signal.rs

//! VectorPort — Vector-based dispatcher (SIGNAL plane)
//!
//! Scope:
//! - VectorPort is a pure codec+dispatch layer for FrameKind.SIGNAL:
//!   it decodes arguments from TLV, invokes a per-vector handler,
//!   and attaches the handler's result to the `body` of the response frame.
//!
//! Selector (sel) contract:
//! - `sel` is delivered verbatim from the received frame (no rewriting).
//! - `sel` meaning is transport-defined; VectorPort treats it as opaque routing metadata.
//!
//! Args/Body contract:
//! - `frame.args` is a pure TLV stream representing the handler arguments.
//! - Only VectorPort/CommandPort (encode_args/decode_args) produces/consumes this format.
//!
//! Handler contract:
//! - Dispatch key is `frame.vec` (Vector).
//! - Handler signature: (sel, args, body) -> Promise<Body | void>
//! - `void` means reply with body=null.
//!
//! Frame ownership / invariants:
//! - VectorPort MUST NOT set/clear FrameFlag.RES (router/runtime owns directionality).
//! - VectorPort does not allocate or interpret `tx_id`, `rid`, `flags`, `opts`.
//! - VectorPort preserves all non-payload fields by echoing the input frame and only
//!   replacing: `errno`, `body`.
//!
//! RID contract:
//! - `rid` is request identity, generated at send-time (typically from vec + request TLV args).
//! - Replies MUST echo the inbound `rid`; reply payload MUST NOT affect `rid`.
//! - Therefore reply `body` is result payload only and is not part of identity.
//!
//! Error mapping:
//! - Unknown Vector: errno=ENOENT, error reply with body=null.
//! - In-handler errors are caught locally, logged, and converted to `body=null` as a last-resort boundary guard.
//!
//! Reliability (ACKREQ):
//! - ACKREQ semantics are owned by worker/runtime (not VectorPort).
//! - VectorPort always returns a reply frame; worker/runtime decides whether it is emitted.
//!
//! wait_next / wait (Pending):
//! - wait_next() returns [sel, body] from the next SIGNAL frame of this tx,
//!   or null on EOF/timeout (stream-friendly for multicast/fanout).
//! - wait_next() is invalid on UNACK (EINVAL).
//! - wait() is a convenience wrapper: null -> ETIMEDOUT (single-result path).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use futures::future::LocalBoxFuture;
pub use futures::FutureExt; // re-export: callers use .boxed_local() when registering handlers

use crate::ipc::device::DeviceUplinkRef;
use crate::ipc::frame::{frame_flag, DataFrame, FrameKind, SignalFrame, TxOptions, Vector};
use crate::io::marshal::tlv::TlvValue;
use crate::lib::kit::array::Bytes;
use crate::rpc::node::{Fanout, is_role_sel, Node, NodeSelector};
use crate::rpc::rid::RID;
use crate::sys::errno;
use crate::sys::error::XputeError;

type DisposerFn = Box<dyn FnOnce()>;

type Args = Vec<TlvValue>;
type Body = Option<Bytes>;

fn encode_args(_args: &Args) -> Bytes {
    let mut writer = crate::io::marshal::tlv::TlvWriter::new();
    writer.write_all(_args);
    writer.finish()
}
fn decode_args(_tlv: &Bytes) -> Args {
    let mut reader = crate::io::marshal::tlv::TlvReader::new(_tlv.clone());
    reader
        .read_all()
        .expect("signal: invalid TLV argument stream")
}

// ============ Pending ============

pub struct Pending {
    tx: DeviceUplinkRef,
    tx_key: i32,
    flags: u32,
}

impl Pending {
    pub fn new(tx: DeviceUplinkRef, tx_key: i32, flags: u32) -> Self {
        Self { tx, tx_key, flags }
    }

    // @internal
    // Low-level waiter. Returns null only on timeout (EOF).
    // Throws on invalid frame types or protocol violations.
    async fn _wait(&self) -> Result<Option<SignalFrame>, XputeError> {
        if (self.flags & frame_flag::ACKREQ) == 0 {
            return Err(XputeError::new(errno::EINVAL, "signal: wait() on unack"));
        }

        let frame = self.tx.wait(self.tx_key).await;
        let Some(frame) = frame else {
            return Ok(None);
        };

        match frame {
            DataFrame::Signal(f) => Ok(Some(f)),
            _ => Err(XputeError::new(errno::EBADMSG, "signal: wait() got non-signal frame")),
        }
    }

    /// Strict waiter: returns only the body.
    /// @throws {XputeError} ETIMEDOUT if the request expires.
    pub async fn wait(&self) -> Result<Body, XputeError> {
        let frame = self._wait().await?;
        let Some(frame) = frame else {
            return Err(XputeError::new(errno::ETIMEDOUT, "signal: wait() timeout"));
        };
        Ok(frame.body)
    }

    /// Permissive waiter: useful for optional responses or polling.
    /// @returns [sender, body] or null if the transaction timed out.
    pub async fn wait_next(&self) -> Result<Option<(NodeSelector, Body)>, XputeError> {
        let frame = self._wait().await?;
        let Some(frame) = frame else {
            return Ok(None);
        };
        Ok(Some((frame.sel, frame.body)))
    }
}

// ============ Request ============

#[derive(Clone)]
pub struct Request {
    tx: DeviceUplinkRef,
    sel: NodeSelector,
    rid: Option<RID>,
    flags: u32,
    opts: TxOptions,
}

impl Request {
    pub fn new(tx: DeviceUplinkRef) -> Self {
        Self {
            tx,
            sel: Node::LOOPBACK,
            rid: None,
            flags: frame_flag::ACKREQ,
            opts: TxOptions::default(),
        }
    }

    /// Set destination selector and optional fanout hint.
    pub fn to(&self, sel: NodeSelector, fanout: Option<u32>) -> Request {
        let fanout = if is_role_sel(sel) { Some(fanout.unwrap_or(Fanout::ONE)) } else { fanout };
        let mut opts = self.opts.clone();
        opts.fanout = fanout;
        Request { tx: self.tx.clone(), sel, rid: self.rid.clone(), flags: self.flags, opts }
    }

    pub fn rid(&self, rid: RID) -> Request {
        Request {
            tx: self.tx.clone(),
            sel: self.sel,
            rid: Some(rid),
            flags: self.flags,
            opts: self.opts.clone(),
        }
    }

    // ---- Flags ----

    /// Fire-and-forget: clears ACKREQ (no RES will be emitted by worker).
    pub fn unack(&self) -> Request {
        Request {
            tx: self.tx.clone(),
            sel: self.sel,
            rid: self.rid.clone(),
            flags: self.flags & !frame_flag::ACKREQ,
            opts: self.opts.clone(),
        }
    }

    pub fn cache(&self) -> Request {
        Request {
            tx: self.tx.clone(),
            sel: self.sel,
            rid: self.rid.clone(),
            flags: self.flags | frame_flag::CACHE,
            opts: self.opts.clone(),
        }
    }

    // ---- Options ----

    pub fn timeout(&self, ms: u32) -> Request {
        let mut opts = self.opts.clone();
        opts.timeout = Some(ms);
        Request {
            tx: self.tx.clone(),
            sel: self.sel,
            rid: self.rid.clone(),
            flags: self.flags,
            opts,
        }
    }

    // ---- Terminators (Actions) ----

    /// Lowest-level call (public; prefer read/write wrappers).
    pub fn send<T>(&self, vec: Vector, args: T, body: Body) -> Pending
    where
        T: Into<Args>,
    {
        let args = args.into();
        let tlv = encode_args(&args);

        let req = SignalFrame {
            kind: FrameKind::Signal,
            sel: self.sel,
            rid: self.rid.clone(),
            vec,
            tx_id: 0, // placeholder
            errno: errno::OK,
            flags: self.flags,
            opts: self.opts.clone(),
            args: tlv,
            body,
        };
        let tx_key = self.tx.send(DataFrame::Signal(req));
        Pending::new(self.tx.clone(), tx_key, self.flags)
    }
}

// ============ Controller ============

type RxHandler =
    dyn Fn(NodeSelector, Args, Body) -> LocalBoxFuture<'static, Result<Body, XputeError>> + 'static;

#[derive(Clone)]
struct RegisteredHandler {
    id: u64,
    handler: Rc<RxHandler>,
}

#[derive(Clone)]
pub struct VectorPort {
    vec_table: Rc<RefCell<HashMap<Vector, RegisteredHandler>>>,
    next_handler_id: Cell<u64>,

    /// Request builder
    pub req: Request,
}

impl VectorPort {
    pub fn new(tx: DeviceUplinkRef) -> Self {
        Self {
            vec_table: Rc::new(RefCell::new(HashMap::new())),
            next_handler_id: Cell::new(1),
            req: Request::new(tx),
        }
    }

    /// Register a receive handler
    pub fn on<F>(&self, vec: Vector, handler: F) -> DisposerFn
    where
        F: Fn(NodeSelector, Args, Body) -> LocalBoxFuture<'static, Result<Body, XputeError>>
            + 'static,
    {
        let id = self.next_handler_id.get();
        self.next_handler_id.set(id + 1);

        if self.vec_table.borrow().contains_key(&vec) {
            web_sys::console::warn_1(&format!("signal: vector overwrite: {vec}").into());
        }

        self.vec_table
            .borrow_mut()
            .insert(vec, RegisteredHandler { id, handler: Rc::new(handler) });

        let vec_table: Weak<RefCell<HashMap<Vector, RegisteredHandler>>> =
            Rc::downgrade(&self.vec_table);

        // disposer: remove only if still mapped to the same handler
        Box::new(move || {
            let Some(vec_table) = vec_table.upgrade() else {
                return;
            };
            let mut vec_table = vec_table.borrow_mut();
            if let Some(entry) = vec_table.get(&vec)
                && entry.id == id
            {
                vec_table.remove(&vec);
            }
        })
    }

    /// Inbound request dispatch.
    ///
    /// - Worker layer (not VectorPort) owns ACKREQ semantics.
    /// - VectorPort always dispatches and returns handler result (or body=null).
    /// - VectorPort/device MUST NOT touch FrameFlag.RES.
    pub async fn receive(&self, frame: SignalFrame) -> SignalFrame {
        let args = decode_args(&frame.args);

        let handler = {
            let vec_table = self.vec_table.borrow();
            vec_table.get(&frame.vec).map(|entry| entry.handler.clone())
        };

        let Some(handler) = handler else {
            web_sys::console::warn_1(&format!("signal: unknown vector: {}", frame.vec).into());
            let mut res = frame.clone();
            res.errno = errno::ENOENT;
            res.body = None;
            return res;
        };

        let res = handler(frame.sel, args, frame.body.clone()).await;

        match res {
            Ok(body) => {
                let mut out = frame.clone();
                out.errno = errno::OK;
                out.body = body;
                out
            }
            Err(err) => {
                web_sys::console::error_1(
                    &format!("signal: {}: {}", frame.vec, err).into(),
                );
                let mut out = frame.clone();
                out.errno = errno::OK;
                out.body = None;
                out
            }
        }
    }

    /// List registered vectors
    pub fn vecs(&self) -> Vec<Vector> {
        self.vec_table.borrow().keys().cloned().collect()
    }
}

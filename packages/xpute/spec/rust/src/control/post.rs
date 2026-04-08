// @xpute/spec/control/post.rs

//! CommandPort — Command-based dispatcher (POST plane)
//!
//! Scope:
//! - CommandPort is a pure codec+dispatch layer for FrameKind.POST:
//!   it decodes arguments from TLV, invokes a per-command handler,
//!   and attaches the handler's result to the `body` of the response frame.
//! - CommandPort does not own transport semantics (routing, reliability,
//!   request/response direction, tx_id allocation).
//!
//! Args/Body contract:
//! - `frame.args` is a pure TLV stream representing the handler arguments.
//! - Only VectorPort/CommandPort (encode_args/decode_args) produces/consumes this format.
//!
//! Handler contract:
//! - Dispatch key is `frame.cmd` (Command).
//! - Handler signature: (args, body) -> Promise<Body | void>
//! - `void` means reply with body=null.
//!
//! Frame ownership / invariants:
//! - CommandPort MUST NOT set/clear FrameFlag.RES (router/runtime owns directionality).
//! - CommandPort does not allocate or interpret `tx_id`, `flags`, `opts`.
//! - CommandPort preserves all non-payload fields by echoing the input frame and only
//!   replacing: `errno`, `body`.
//!
//! Error mapping:
//! - Unknown Command: errno=ENOENT, error reply with body=null.
//! - In-handler errors are caught locally, logged, and converted to `body=null` as a last-resort boundary guard.
//!
//! Reliability (ACKREQ):
//! - ACKREQ semantics are owned by worker/runtime (not CommandPort).
//! - CommandPort always returns a reply frame; worker/runtime decides whether it is emitted.
//!
//! wait (Pending):
//! - Shell is not a multicast/fanout plane: each tx is expected to yield at most one reply.
//! - wait() returns body from the next POST frame of this tx.
//! - wait() is invalid on UNACK (EINVAL).
//! - wait() expects FrameKind.POST; non-POST => EBADMSG.
//! - wait() timeout/null -> ETIMEDOUT.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use futures::future::LocalBoxFuture;
pub use futures::FutureExt; // re-export: callers use .boxed_local() when registering handlers

use crate::control::cmd::Command;
use crate::ipc::device::DeviceUplinkRef;
use crate::ipc::frame::{frame_flag, DataFrame, FrameKind, PostFrame, TxOptions};
use crate::io::marshal::tlv::TlvValue;
use crate::lib::kit::array::Bytes;
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
        .expect("post: invalid TLV argument stream")
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

    async fn _wait(&self) -> Result<Option<PostFrame>, XputeError> {
        if (self.flags & frame_flag::ACKREQ) == 0 {
            return Err(XputeError::new(errno::EINVAL, "post: wait() on unack"));
        }

        let frame = self.tx.wait(self.tx_key).await;
        let Some(frame) = frame else {
            return Ok(None);
        };

        match frame {
            DataFrame::Post(frame) => Ok(Some(frame)),
            _ => Err(XputeError::new(errno::EBADMSG, "post: wait() got non-post frame")),
        }
    }

    pub async fn wait(&self) -> Result<Body, XputeError> {
        let frame = self._wait().await?;
        let Some(frame) = frame else {
            return Err(XputeError::new(errno::ETIMEDOUT, "post: wait() timeout"));
        };

        Ok(frame.body)
    }
}

// ============ Request ============

#[derive(Clone)]
pub struct Request {
    tx: DeviceUplinkRef,
    flags: u32,
    opts: TxOptions,
}

impl Request {
    pub fn new(tx: DeviceUplinkRef) -> Self {
        Self {
            tx,
            flags: frame_flag::ACKREQ,
            opts: TxOptions::default(),
        }
    }

    // ---- Flags ----

    /// Fire-and-forget: clears ACKREQ (no RES will be emitted by worker).
    pub fn unack(&self) -> Request {
        Request {
            tx: self.tx.clone(),
            flags: self.flags & !frame_flag::ACKREQ,
            opts: self.opts.clone(),
        }
    }

    // ---- Options ----

    pub fn timeout(&self, ms: u32) -> Request {
        let mut opts = self.opts.clone();
        opts.timeout = Some(ms);

        Request {
            tx: self.tx.clone(),
            flags: self.flags,
            opts,
        }
    }

    // ---- Terminators (Actions) ----

    /// Lowest-level call (public; prefer read/write wrappers).
    pub fn send<T>(&self, cmd: Command, args: T, body: Body) -> Pending
    where
        T: Into<Args>,
    {
        let args = args.into();
        let tlv = encode_args(&args);

        let req = PostFrame {
            kind: FrameKind::Post,
            cmd,
            tx_id: 0, // placeholder
            errno: errno::OK,
            flags: self.flags,
            opts: self.opts.clone(),
            args: tlv,
            body,
        };

        let tx_key = self.tx.send(DataFrame::Post(req));
        Pending::new(self.tx.clone(), tx_key, self.flags)
    }
}

// ============ Controller ============

type RxHandler =
    dyn Fn(Args, Body) -> LocalBoxFuture<'static, Result<Body, XputeError>> + 'static;

#[derive(Clone)]
struct RegisteredHandler {
    id: u64,
    handler: Rc<RxHandler>,
}

#[derive(Clone)]
pub struct CommandPort {
    cmd_table: Rc<RefCell<HashMap<Command, RegisteredHandler>>>,
    next_handler_id: Cell<u64>,

    /// Request builder
    pub req: Request,
}

impl CommandPort {
    pub fn new(tx: DeviceUplinkRef) -> Self {
        Self {
            cmd_table: Rc::new(RefCell::new(HashMap::new())),
            next_handler_id: Cell::new(1),
            req: Request::new(tx),
        }
    }

    /// Register a receive handler
    pub fn on<F>(&self, cmd: Command, handler: F) -> DisposerFn
    where
        F: Fn(Args, Body) -> LocalBoxFuture<'static, Result<Body, XputeError>> + 'static,
    {
        let id = self.next_handler_id.get();
        self.next_handler_id.set(id + 1);

        if self.cmd_table.borrow().contains_key(&cmd) {
            web_sys::console::warn_1(&format!("post: command overwrite: '{cmd}'").into());
        }

        self.cmd_table.borrow_mut().insert(
            cmd.clone(),
            RegisteredHandler {
                id,
                handler: Rc::new(handler),
            },
        );

        let cmd_table: Weak<RefCell<HashMap<Command, RegisteredHandler>>> =
            Rc::downgrade(&self.cmd_table);

        // disposer: remove only if still mapped to the same handler
        Box::new(move || {
            let Some(cmd_table) = cmd_table.upgrade() else {
                return;
            };

            let mut cmd_table = cmd_table.borrow_mut();
            if let Some(entry) = cmd_table.get(&cmd)
                && entry.id == id
            {
                cmd_table.remove(&cmd);
            }
        })
    }

    /// Inbound request dispatch.
    ///
    /// - Worker layer (not CommandPort) owns ACKREQ semantics.
    /// - CommandPort always dispatches and returns handler result (or body=null).
    /// - Worker decides whether to emit RES frame based on ACKREQ flag.
    pub async fn receive(&self, frame: PostFrame) -> PostFrame {
        let args = decode_args(&frame.args);

        let handler = {
            let cmd_table = self.cmd_table.borrow();
            cmd_table.get(&frame.cmd).map(|entry| entry.handler.clone())
        };

        let Some(handler) = handler else {
            web_sys::console::warn_1(&format!("post: unknown command: {}", frame.cmd).into());

            let mut res = frame.clone();
            res.errno = errno::ENOENT;
            res.body = None;
            return res;
        };

        let res = handler(args, frame.body.clone()).await;

        match res {
            Ok(body) => {
                let mut out = frame.clone();
                out.errno = errno::OK;
                out.body = body;
                out
            }
            Err(err) => {
                web_sys::console::error_1(&format!("post: {}: {}", frame.cmd, err).into());

                let mut out = frame.clone();
                out.errno = errno::OK;
                out.body = None;
                out
            }
        }
    }

    /// List registered commands
    pub fn cmds(&self) -> Vec<Command> {
        self.cmd_table.borrow().keys().cloned().collect()
    }
}

// @pxl/engine/lib.rs

use std::rc::Rc;

use futures::future::LocalBoxFuture;
use js_sys::{Array, Promise};
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{future_to_promise, JsFuture};

mod modules;

use xpute_spec::control::post::CommandPort;
use xpute_spec::control::signal::VectorPort;
use xpute_spec::ipc::device::{DeviceUplink, DeviceUplinkRef};
use xpute_spec::ipc::frame::{DataFrame, FrameKind, PostFrame, SignalFrame};
use xpute_spec::sys::errno;

// ============ Logging macro ============

macro_rules! log {
    ($($t:tt)*) => {
        web_sys::console::log_1(&format!($($t)*).into())
    };
}

// ============ JS DeviceUplink extern binding ============

/// Extern binding for the JS-side DeviceUplink object provided by the worker runtime.
///
/// JS interface (from @xpute/spec/ipc/link.ts):
///   send(frame: DataFrame): i32       — enqueue a frame; returns tx_key (≥0) or negative errno
///   wait(tx_key: i32): Promise<DataFrame | null>  — wait for ACK frame or null on EOF
///
/// The runtime (TxController in tx.ts) owns tx_id assignment, ACKREQ semantics,
/// and timeout-based cleanup. Rust only calls send/wait; it does not interpret tx_id.
#[wasm_bindgen]
extern "C" {
    type JsUplink;

    /// Enqueue/emit a data-plane frame. Returns tx_key (≥0) or negative errno.
    /// The JS runtime (TxController) assigns tx_id on the object in-place before forwarding.
    #[wasm_bindgen(method, catch)]
    fn send(this: &JsUplink, frame: JsValue) -> Result<i32, JsValue>;

    /// Wait for the next ACK frame for tx_key.
    /// Resolves to a DataFrame object or null on EOF/timeout.
    /// Rejects with XputeError on protocol violation or double-wait.
    #[wasm_bindgen(method)]
    fn wait(this: &JsUplink, tx_key: i32) -> Promise;
}

// ============ DeviceUplink bridge ============

/// Wraps the JS DeviceUplink JsValue and implements the Rust DeviceUplink trait.
/// Frame serialization/deserialization is handled by serde-wasm-bindgen.
struct RustJsUplink(JsUplink);

impl DeviceUplink for RustJsUplink {
    fn send(&self, frame: DataFrame) -> i32 {
        // Serialize each variant directly — DataFrame enum itself is not serialized
        // to avoid the Rust-default tagged-enum shape `{"Signal": {...}}`.
        let js = match &frame {
            DataFrame::Signal(sf) => serde_wasm_bindgen::to_value(sf),
            DataFrame::Post(pf) => serde_wasm_bindgen::to_value(pf),
        };
        match js {
            Ok(js) => self.0.send(js).unwrap_or(errno::EFAULT),
            Err(_) => errno::EBADMSG,
        }
    }

    /// Returns `LocalBoxFuture<'static, Option<DataFrame>>`.
    /// Awaits the JS Promise returned by `tx.wait(tx_key)`.
    /// Returns None if the promise resolves to null/undefined (EOF/timeout)
    /// or rejects (e.g. ETIMEDOUT from TxController).
    fn wait(&self, tx_key: i32) -> LocalBoxFuture<'static, Option<DataFrame>> {
        let promise = self.0.wait(tx_key);
        Box::pin(async move {
            match JsFuture::from(promise).await {
                Ok(v) if v.is_null() || v.is_undefined() => None,
                Ok(v) => js_to_data_frame(v).ok(),
                Err(_) => None,
            }
        })
    }
}

// ============ Engine ============

#[wasm_bindgen]
pub struct Engine {
    signal: VectorPort,
    post: CommandPort,
}

#[wasm_bindgen]
impl Engine {
    /// Construct the engine with a JS DeviceUplink (`tx`).
    ///
    /// `tx` must be a JS object implementing DeviceUplink (TxController in tx.ts).
    /// VectorPort and CommandPort are wired up to this uplink for outbound frames.
    #[wasm_bindgen(constructor)]
    pub fn new(tx: JsValue) -> Engine {
        log!("Engine::new");

        let uplink: DeviceUplinkRef = Rc::new(RustJsUplink(tx.unchecked_into::<JsUplink>()));
        let mut engine = Engine {
            signal: VectorPort::new(uplink.clone()),
            post: CommandPort::new(uplink),
        };
        engine.load_modules();
        engine
    }

    fn load_modules(&mut self) {
        log!("Engine::load_modules");
        use xpute_spec::ipc::device::DeviceEnvironment;
        let env = DeviceEnvironment {
            signal: self.signal.clone(),
            post: self.post.clone(),
        };
        modules::page::load(&env);

        #[cfg(test)]
        {
            use xpute_spec::control::post::FutureExt;
            let _disposer = self.post.on("echo".into(), |_args, body| {
                async move { Ok(body) }.boxed_local()
            });
        }
    }

    /// Returns the list of SIGNAL vectors this engine handles.
    pub fn capable_vecs(&self) -> Array {
        let vecs = self.signal.vecs();
        let arr = Array::new_with_length(vecs.len() as u32);
        for (i, v) in vecs.iter().enumerate() {
            arr.set(i as u32, JsValue::from_f64(*v as f64));
        }
        arr
    }

    /// Returns the list of POST commands this engine handles.
    pub fn capable_cmds(&self) -> Array {
        let cmds = self.post.cmds();
        let arr = Array::new_with_length(cmds.len() as u32);
        for (i, c) in cmds.iter().enumerate() {
            arr.set(i as u32, JsValue::from_str(c));
        }
        arr
    }

    /// Dispatch an inbound frame to the appropriate port (VectorPort for SIGNAL,
    /// CommandPort for POST). Returns a Promise resolving to the response frame.
    ///
    /// Frame ownership contract (from worker.ts / frame.ts):
    /// - Engine MUST NOT set/clear FrameFlag.RES (owned by runtime).
    /// - Engine preserves tx_id, flags, opts, sel, rid verbatim.
    /// - Engine only replaces: errno, body.
    pub fn receive(&self, frame: JsValue) -> Promise {
        // Clone VectorPort/CommandPort into the async closure.
        // Both are Rc-backed; clones share the same dispatch tables.
        let signal = self.signal.clone();
        let post = self.post.clone();

        future_to_promise(async move {
            match js_to_data_frame(frame)? {
                DataFrame::Signal(sf) => {
                    let response = signal.receive(sf).await;
                    serde_wasm_bindgen::to_value(&response)
                        .map_err(|e| JsValue::from_str(&e.to_string()))
                }
                DataFrame::Post(pf) => {
                    let response = post.receive(pf).await;
                    serde_wasm_bindgen::to_value(&response)
                        .map_err(|e| JsValue::from_str(&e.to_string()))
                }
            }
        })
    }
}

// ============ Frame deserialization ============

/// Peek only the `kind` discriminant before full deserialization.
#[derive(Deserialize)]
struct FrameKindPeek {
    kind: FrameKind,
}

/// Deserialize a flat JS frame object `{kind, tx_id, ...}` into the correct DataFrame variant.
///
/// serde-wasm-bindgen notes (default serializer, not json_compatible):
/// - `Vec<u8>` (args/body) ↔ Uint8Array
/// - `u64` (NodeSelector/sel) ↔ BigInt
/// - `Option<T>` → undefined when None; null/undefined → None
fn js_to_data_frame(v: JsValue) -> Result<DataFrame, JsValue> {
    let peek: FrameKindPeek = serde_wasm_bindgen::from_value(v.clone())
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    match peek.kind {
        FrameKind::Signal => {
            let sf: SignalFrame = serde_wasm_bindgen::from_value(v)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            Ok(DataFrame::Signal(sf))
        }
        FrameKind::Post => {
            let pf: PostFrame = serde_wasm_bindgen::from_value(v)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            Ok(DataFrame::Post(pf))
        }
        other => Err(JsValue::from_str(&format!(
            "js_to_data_frame: unhandled kind {:?}",
            other
        ))),
    }
}

// ============ Tests ============

#[cfg(test)]
mod tests {
    use wasm_bindgen::JsValue;
    use wasm_bindgen_futures::JsFuture;
    use wasm_bindgen_test::*;

    use xpute_spec::ipc::frame::{FrameKind, PostFrame};
    use xpute_spec::sys::errno;

    use super::Engine;

    /// POST round-trip: echo handler returns body unchanged.
    ///
    /// Uplink is null because the echo handler never calls req.send().
    /// Verifies: JS frame → Rust dispatch → handler → JS response, errno=OK, body preserved.
    #[wasm_bindgen_test]
    async fn post_echo_roundtrip() {
        let engine = Engine::new(JsValue::NULL);

        let frame = serde_wasm_bindgen::to_value(&PostFrame {
            kind: FrameKind::Post,
            tx_id: 1,
            errno: errno::OK,
            flags: 0,
            opts: Default::default(),
            args: vec![],
            body: Some(b"hello".to_vec()),
            cmd: "echo".into(),
        })
        .unwrap();

        let result = JsFuture::from(engine.receive(frame)).await.unwrap();
        let response: PostFrame = serde_wasm_bindgen::from_value(result).unwrap();

        assert_eq!(response.errno, errno::OK);
        assert_eq!(response.body, Some(b"hello".to_vec()));
        assert_eq!(response.cmd, "echo");
    }
}

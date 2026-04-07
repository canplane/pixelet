// @pxl/engine/lib.rs

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;
use js_sys::{Array, Promise, Reflect};

macro_rules! log {
    ($($t:tt)*) => {
        web_sys::console::log_1(&format!($($t)*).into())
    };
}

#[wasm_bindgen]
pub struct Engine {
    tx: JsValue,
}

#[wasm_bindgen]
impl Engine {
    #[wasm_bindgen(constructor)]
    pub fn new(tx: JsValue) -> Engine {
        log!("Engine::new");
        web_sys::console::log_1(&tx);

        let mut engine = Engine { tx };
        engine.load_modules();
        engine
    }

    fn load_modules(&mut self) {
        log!("Engine::load_modules");
        // TODO: page_module::load(...)
    }

    pub fn capable_vecs(&self) -> Array {
        Array::new()
    }

    pub fn capable_cmds(&self) -> Array {
        Array::new()
    }

    pub fn receive(&self, frame: JsValue) -> Promise {
        future_to_promise(async move {
            let kind = Reflect::get(&frame, &JsValue::from_str("kind"))?;

            // TS:
            // switch (frame.kind) {
            //   case FrameKind.SIGNAL:
            //   case FrameKind.POST:
            // }

            web_sys::console::log_2(&"Engine::receive kind =".into(), &kind);

            Ok(frame)
        })
    }
}
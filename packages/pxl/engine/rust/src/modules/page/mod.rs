// @pxl/engine/src/modules/page/mod.rs

#[cfg(feature = "client")]
use wasm_bindgen_futures::spawn_local;

#[cfg(feature = "client")]
use xpute_spec::control::cmd;
#[cfg(feature = "client")]
use xpute_spec::control::post::FutureExt;
#[cfg(feature = "client")]
use xpute_spec::control::vec::Vector;
use xpute_spec::ipc::device::DeviceEnvironment;
#[cfg(feature = "client")]
use xpute_spec::io::marshal::tlv::TlvValue;
#[cfg(feature = "client")]
use xpute_spec::rpc::node::Node;

/// IVec: voxel chunk index. Placeholder until a proper 3D integer vector type is introduced.
#[cfg(feature = "client")]
pub type IVec = u64;

// ============ Client ============
// Receives page demands, fetches terrain from the server, and relays results back to the local shell.

#[cfg(feature = "client")]
fn load_client(env: &DeviceEnvironment) {
    let signal = env.signal.clone();
    let post = env.post.clone();

    let _ = post.clone().on(cmd::TILE.into(), move |args, _body| {
        let signal = signal.clone();
        let post = post.clone();

        async move {
            // Decode IVec[] from TLV args
            let demands: Vec<IVec> = args
                .iter()
                .filter_map(|v| if let TlvValue::U64(n) = v { Some(*n) } else { None })
                .collect();

            // Request each page independently and relay results as they arrive.
            for page_idx in demands {
                let signal = signal.clone();
                let post = post.clone();

                spawn_local(async move {
                    match signal
                        .req
                        .to(Node::SERVER, None)
                        .send(Vector::TILE, [TlvValue::U64(page_idx)], None)
                        .wait()
                        .await
                    {
                        Ok(terrain_encoded) => {
                            post.req
                                .unack()
                                .send(cmd::TILE.into(), [TlvValue::U64(page_idx)], terrain_encoded);
                        }
                        Err(err) => {
                            web_sys::console::error_1(
                                &format!(
                                    "[{}/{}] core: failed to stream tile: {}",
                                    cmd::TILE,
                                    page_idx,
                                    err
                                )
                                .into(),
                            );
                            post.req
                                .unack()
                                .send(cmd::TILE.into(), [TlvValue::U64(page_idx)], None);
                        }
                    }
                });
            }

            Ok(None) // handler returns immediately; spawned tasks relay independently
        }
        .boxed_local()
    });
}

/// Module entry point.
#[allow(unused_variables)]
pub fn load(env: &DeviceEnvironment) {
    #[cfg(feature = "client")]
    load_client(env);
}

// pxl_core/render/gltf/load.rs

//! Shared loading and caching of the models and the skins, so any consumer —
//! the companions, the decor kinds, the picker's preview — hits the network and
//! the parser at most once per URL.
//!
//! The fetch is the shell's: a URL asked for here for the first time joins the
//! request lane (`take_load_requests`), and the shell answers with the bytes of
//! a model (`gltf_arrived`, parsed here) or the handle of an image it decoded
//! and holds (`image_arrived`) — the file's own pixels, top row first, no
//! colour conversion and no premultiplied alpha. A consumer polls: None while
//! the load is in flight or has failed.

use std::collections::HashMap;
use std::rc::Rc;

use super::gltf::{parse_gltf, Gltf};
use crate::global::Global;
use crate::module::world::change::request_world_tick;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadKind {
    Gltf,
    Image,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadRequest {
    pub kind: LoadKind,
    pub url: String,
}

enum Entry<T> {
    Pending,
    Ready(T),
    Failed,
}

struct Loader {
    gltf: HashMap<String, Entry<Rc<Gltf>>>,
    image: HashMap<String, Entry<u32>>,
    requests: Vec<LoadRequest>,
    revision: u32,
}

static LOADER: Global<Loader> = Global::new();

fn loader() -> &'static mut Loader {
    LOADER.get(|| Loader {
        gltf: HashMap::new(),
        image: HashMap::new(),
        requests: Vec::new(),
        revision: 0,
    })
}

/// A parsed glTF by URL, once it has arrived.
pub fn load_gltf(url: &str) -> Option<Rc<Gltf>> {
    let l = loader();
    match l.gltf.get(url) {
        Some(Entry::Ready(g)) => Some(g.clone()),
        Some(_) => None,
        None => {
            l.gltf.insert(url.to_string(), Entry::Pending);
            l.requests.push(LoadRequest { kind: LoadKind::Gltf, url: url.to_string() });
            None
        }
    }
}

/// A skin by URL, as the shell's image handle, once it has arrived.
pub fn load_image(url: &str) -> Option<u32> {
    let l = loader();
    match l.image.get(url) {
        Some(Entry::Ready(h)) => Some(*h),
        Some(_) => None,
        None => {
            l.image.insert(url.to_string(), Entry::Pending);
            l.requests.push(LoadRequest { kind: LoadKind::Image, url: url.to_string() });
            None
        }
    }
}

/// Whether a load has failed: it is never asked for again.
pub fn load_failed(url: &str) -> bool {
    let l = loader();
    matches!(l.gltf.get(url), Some(Entry::Failed)) || matches!(l.image.get(url), Some(Entry::Failed))
}

/// Moves on every arrival and every failure.
pub fn load_revision() -> u32 {
    loader().revision
}

/// The URLs to fetch, drained by the shell.
pub fn take_load_requests() -> Vec<LoadRequest> {
    core::mem::take(&mut loader().requests)
}

/// The shell's bytes for a model: parsed, or failed when they do not parse.
pub fn gltf_arrived(url: &str, bytes: &[u8]) {
    let l = loader();
    let entry = match parse_gltf(bytes, &[]) {
        Ok(g) => Entry::Ready(Rc::new(g)),
        Err(_) => Entry::Failed,
    };
    l.gltf.insert(url.to_string(), entry);
    l.revision += 1;
    request_world_tick();
}

/// The shell's handle for a decoded image.
pub fn image_arrived(url: &str, handle: u32) {
    let l = loader();
    l.image.insert(url.to_string(), Entry::Ready(handle));
    l.revision += 1;
    request_world_tick();
}

/// The shell's report that a fetch failed.
pub fn load_rejected(url: &str) {
    let l = loader();
    if l.gltf.contains_key(url) {
        l.gltf.insert(url.to_string(), Entry::Failed);
    }
    if l.image.contains_key(url) {
        l.image.insert(url.to_string(), Entry::Failed);
    }
    l.revision += 1;
}

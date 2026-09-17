// pxl_render/lib.rs

//! pixelet's view: what the model (pxl-shared) looks like when drawn — the
//! voxel volumes and the meshes a chunk bakes into, the terrain's elevation,
//! the LOD ladder, the models' glTF, the billboards' sheet, the draw list,
//! the shaders and pipelines — and the device that draws it. It knows the
//! model and nothing of the kernel (pxl-core): the kernel decides what and
//! when, and hands a frame what it reads (`gpu::renderer::FrameInput`).

pub mod billboard;
pub mod draw;
pub mod environment;
pub mod gltf;
pub mod gpu;
pub mod layer;
pub mod lod;
pub mod marker;
pub mod shader;
pub mod tex_array;

/// Serializes the native checks that touch the view's globals: they run on
/// several threads, and a `Global` is for one.
#[cfg(test)]
pub fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

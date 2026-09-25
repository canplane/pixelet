// pxl-render/lib.rs

//! pixelet's view: what the model (pxl-spec) looks like when drawn — the
//! voxel volumes and the meshes a chunk bakes into, the terrain's elevation,
//! the LOD criterion's reference, the models' glTF, the billboards' sheet,
//! the draw list, the shaders and pipelines — and the device that draws it. It knows the
//! model and nothing of the kernel (pxl-kernel): the kernel decides what and
//! when, and hands a frame what it reads (`gpu::renderer::FrameInput`).

pub mod asset;
pub mod frame;
pub mod gpu;
pub mod look;
pub mod mesh;

/// Serializes the native checks that touch the view's globals: they run on
/// several threads, and a `Global` is for one.
#[cfg(test)]
pub fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

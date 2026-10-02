// pxl-render/lib.rs

//! pixelet's view: what the model looks like drawn, and the device that draws
//! it. It knows the model and nothing of the kernel.

pub mod asset;
pub mod frame;
pub mod gpu;
pub mod look;
pub mod mesh;

/// The native tests run on several threads, and a `Global` is for one.
#[cfg(test)]
pub fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

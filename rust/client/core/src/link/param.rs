// pxl-core/link/param.rs

//! The numbers of the core's links to its host.

/// After the GPU's device is refused, the core asks again after
/// `DEVICE_RETRY_BASE_MS`, doubled for each refusal in a row and never more
/// than `DEVICE_RETRY_CAP_MS`, and at once when the app is shown again
/// (link/gpu.rs `hold`). A platform may answer no adapter for a while, as
/// after its GPU process ends, and it tells the core nothing when one is
/// back.
pub(super) const DEVICE_RETRY_BASE_MS: f64 = 1_000.0;
pub(super) const DEVICE_RETRY_CAP_MS: f64 = 30_000.0;

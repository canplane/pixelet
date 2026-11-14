// pxl-core/spec.rs

//! The core's values its specs hold (spec/client/core/), each written from
//! its spec by the generator and read as `crate::spec::<name>`.

#[path = "spec/demand.spec.rs"]
pub mod demand;
#[path = "spec/file.spec.rs"]
pub mod file;
#[path = "spec/gizmo.spec.rs"]
pub mod gizmo;
#[path = "spec/manifest.spec.rs"]
pub mod manifest;
#[path = "spec/perf.spec.rs"]
pub mod perf;
#[path = "spec/phase.spec.rs"]
pub mod phase;
#[path = "spec/schema.spec.rs"]
pub mod schema;
#[path = "spec/step.spec.rs"]
pub mod step;

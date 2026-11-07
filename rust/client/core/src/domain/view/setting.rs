// pxl-core/domain/view/setting.rs

//! Every setting, declared once in
//! spec/client/core/domain/view/setting/schema.json and held in one store
//! (`store.rs`). It sits under the view, the lowest part that reads it, and
//! any part above may read it.

#[path = "setting/schema.spec.rs"]
pub mod schema_spec;
pub mod store;

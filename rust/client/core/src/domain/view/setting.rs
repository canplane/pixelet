// pxl-core/domain/view/setting.rs

//! The settings. Each is declared once in
//! spec/client/core/domain/view/setting/schema.json and held in one store
//! (`store.rs`). They sit under the view because the view is the lowest part
//! that reads them, and any part above the view may read them too.

#[path = "setting/schema.spec.rs"]
pub mod schema_spec;
pub mod store;

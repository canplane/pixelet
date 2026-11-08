// pxl-core/domain/setting.rs

//! The settings. Each is declared once in
//! spec/client/core/domain/setting/schema.json and held in one store
//! (`store.rs`). They stand under both heads, which read them, as the top
//! does (pxl-tool rule.rs).

#[path = "setting/schema.spec.rs"]
pub mod schema_spec;
pub mod store;

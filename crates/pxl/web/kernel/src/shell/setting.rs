// pxl-kernel/shell/setting.rs

//! Every setting the app has: declared once in schema.json, generated into
//! both languages from it, and held in one store the sheet edits and the
//! drawer reads. A setting exists here or it does not exist.

pub mod schema;
#[path = "setting/schema.spec.rs"]
pub mod schema_spec;
pub mod store;

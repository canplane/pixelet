// pxl-tool/spec.rs

//! Writes the TypeScript and Rust generated from `spec/` JSON: its data
//! specs (data.rs) and its command table (cmd.rs) through xpute's generator,
//! and what else it generates beside them. It links none of the crates it
//! writes into, so it builds when their generated code is broken.

pub mod asset;
pub(crate) mod bake;
pub mod catalog;
pub mod cmd;
pub mod data;
pub mod ingest;
pub mod route;
pub mod setting;

/// What xpute's generator writes and reads with, shared: the generated file,
/// the JSON's tree, and the text helpers.
pub use xpute_tool::spec::{
    align_star, casing_note, enum_name, file, js_num, json_str, line_comments, pascal, rgb,
    rgb_literal, rs_doc, rs_ident, snake, ts_doc, Node, Output,
};

use serde::Deserialize;
use xpute_tool::spec::generate_at;

use crate::root;

pub fn outputs() -> Result<Vec<Output>, String> {
    let mut out = Vec::new();
    out.extend(cmd::outputs()?);
    out.extend(setting::outputs()?);
    out.extend(asset::color_outputs()?);
    out.extend(asset::cube_outputs()?);
    out.extend(asset::decor_outputs()?);
    out.extend(bake::publication_outputs()?);
    out.extend(bake::object_outputs()?);
    out.extend(bake::place_theme_outputs()?);
    out.extend(data::outputs()?);
    out.extend(ingest::outputs()?);
    out.extend(catalog::outputs()?);
    out.extend(route::outputs()?);
    Ok(out)
}

/// The trees whose generated files another generator writes: xpute's, by
/// xpute-tool.
const OTHERS: [&str; 1] = ["xpute/"];

/// Writes every generated file that differs from what its JSON writes and deletes every one
/// that nothing writes any more; where `write` is false, it fails wherever it finds either
/// instead.
pub fn generate(write: bool) -> Result<(), String> {
    generate_at(&root(), outputs()?, &OTHERS, write)
}

pub fn read(path: &str) -> Result<Node, String> {
    read_as(path)
}

pub(crate) fn read_as<T: for<'de> Deserialize<'de>>(path: &str) -> Result<T, String> {
    let text = std::fs::read_to_string(root().join(path)).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
}

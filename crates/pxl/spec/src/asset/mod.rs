// pxl-spec/asset/mod.rs

pub mod ingested;
pub mod texture;

pub use texture::*;

#[cfg(test)]
#[path = "asset.test.rs"]
mod asset_test;

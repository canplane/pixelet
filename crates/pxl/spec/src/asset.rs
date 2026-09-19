// pxl-spec/asset.rs

pub mod ingested;
pub mod texture;

pub use texture::*;

#[cfg(test)]
#[path = "asset.test.rs"]
mod test;

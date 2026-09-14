// pxl_shared/record/mod.rs

pub mod ingested;
pub mod palette;
pub mod placement;
pub mod shipped;
pub mod table;
pub mod texture;

pub use palette::*;
pub use placement::*;
pub use shipped::*;
pub use table::*;
pub use texture::*;

#[cfg(test)]
#[path = "record.test.rs"]
mod record_test;

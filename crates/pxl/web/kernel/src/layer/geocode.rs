// pxl-kernel/layer/geocode.rs

//! What the place under the map centre is called: the dictionary of every
//! place the country has, the group of pages the centre is in, and the two
//! fetches that keep them.
//!
//! Nothing here is drawn. The label is read out of the packets where they
//! lie and written into the UI snapshot, so the layer holds bytes and a
//! wishlist and nothing else.

pub mod fetcher;
pub mod repository;
pub mod store;

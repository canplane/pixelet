// pxl-spec/limit.rs

//! Where the world ends, one place for every limit an edit and a column
//! meet: the world is open from `BEDROCK_DEPTH` under the ground or sea
//! level, whichever is lower, to `HEADROOM` over the ground or sea level,
//! whichever is higher, in leaf cells (pxl-content strata.rs `bedrock`,
//! `ceiling`). Under the floor is bedrock, the unknown the world holds no more of
//! (`spec/spec/asset/cube.json` `unknown`), and the service area is walled by
//! it from inside: a leaf column with ground beside one past the area is
//! bedrock up to its own face, a cell of the leaves as any is, so the wall's
//! outer face stands on the area's edge at every zoom (pxl-engine
//! area.rs `laid`). A walker keeps off the wall's columns and an edit off
//! them, by the rule the wall is laid by (pxl-kernel domain/query/limit.rs
//! `within`, `editable`).

pub use pxl_content::strata::{bedrock, ceiling};

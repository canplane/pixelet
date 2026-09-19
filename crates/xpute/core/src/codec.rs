// xpute-core/codec.rs

//! The pair of packages/xpute/core/src/codec, less what Rust has as a crate:
//! base64url is `base64`, hex is `hex`, FNV is `fnv`, and `json` is what is
//! left — the host's JSON, which no crate writes.

pub mod json;

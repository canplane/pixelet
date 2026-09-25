// pxl-kernel/layer/billboard/order.rs

/// Draw order among billboard sets (crates/pxl/web/kernel/src/layer/billboard/field.rs), for
/// the case two billboards land on the same pixels at the same depth. Depth
/// testing settles everything else; this only settles ties, and the tie is
/// between a badge that labels a fixed place and a bubble somebody is saying
/// right now. The bubble wins — it is transient and the badge will still be
/// there afterward.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum BillboardOrder {
    Poi = 0,
    Chat = 1,
    Debug = 2,
}

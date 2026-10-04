// pxl-kernel/layer/billboard/order.rs

/// Draw order among billboard sets, only for ties on the same pixels at the
/// same depth: the transient bubble wins over the badge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum BillboardOrder {
    Poi = 0,
    Chat = 1,
}

/// One set per order, each owner letting its set go before making another.
pub const SETS_MAX: usize = 2;

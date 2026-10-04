// pxl-kernel/layer/billboard/order.rs

/// Draw order among billboard sets, only for ties on the same pixels at the
/// same depth: the transient bubble wins over the badge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum BillboardOrder {
    Poi = 0,
    Chat = 1,
    Debug = 2,
}

/// Billboard sets live at once at most: one an order, since each order's
/// owner — the badges, the companion's bubble, the gizmos' labels — holds one
/// set and lets it go before it makes another (field.rs `new_set`).
pub const SETS_MAX: usize = 3;

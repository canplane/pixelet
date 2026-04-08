// @xpute/spec/ipc/topology.rs

// ---- Workers ----

/// tx_id layout: [ worker_id (8b) | tx_key (24b) ]
/// 24-bit sequence provides 16.7M unique keys per worker before wrap.
pub const WORKER_ID_W: u32 = 8;
pub const WORKER_ID_CAP: u32 = 1 << WORKER_ID_W; // 256
pub const WORKER_ID_SHAMT: u32 = 32 - WORKER_ID_W; // 24

/// Low-24-bit tx_key mask extracted from tx_id.
pub const SEQ_MASK: u32 = (1 << WORKER_ID_SHAMT) - 1;

pub type DevicePort = u16;
pub type WorkerId = u8;

/// WorkerID constants.
pub mod worker_id {
    use super::WorkerId;

    pub const MAIN: WorkerId = 1;  // Main thread (I/O bound)
    pub const CORE: WorkerId = 2;  // CPU worker

    /// Guest linker — virtual worker, not listed in runtime topology.
    pub const GUEST: WorkerId = (super::WORKER_ID_CAP - 1) as WorkerId;
}

/// DevicePort constants.
///
/// Note: DeviceType (JS / BINDGEN) and DeviceSpec are TS/manifest-level constructs.
/// Device loading is owned by the JS runtime; they are not ported to Rust.
pub mod device_port {
    use super::DevicePort;

    pub const ENGINE: DevicePort = 1;       // Rust WASM engine (BINDGEN)
    pub const ENGINE_TS: DevicePort = 2;    // TypeScript engine (legacy JS)
    pub const ENGINE_SPARE: DevicePort = 3;
}

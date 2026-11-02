// pxl-content/param.rs

//! gpu-portable: what pixelet's content is, as it sets it: each value one
//! literal, a subject a module, its reason beside it, as the engine's
//! (pxl-engine param.rs) are.

pub mod cloud {
    /// A lattice unit is 2^12 cells, about 8 km at mid latitudes; a power of two
    /// so the clouds wrap with the map.
    pub const LATTICE_SHAMT: u32 = 12;

    /// In cells a second, about 50 m/s at mid latitudes, a jet stream's: at a
    /// breeze's 8 a cloud a kilometer wide takes minutes to cross its width and
    /// reads as still. The field moves against it, so a point on the ground sees
    /// the field at itself plus the drift.
    pub const WIND: [f64; 2] = [24.0, 10.0];

    /// A fair-weather cumulus base.
    pub const CUMULUS_KM: f64 = 1.5;
}

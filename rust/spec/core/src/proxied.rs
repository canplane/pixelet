// pxl-spec-core/proxied.rs

//! gpu-portable: the cast a rasterized proxy starts
//! (doc/minutes/20261024-a-proxy-only-starts-the-ray.md): which of the
//! cast's dispatches a frame runs, laid on the GPU off the cut's counts, so
//! no frame waits on a read of them (pxl-shader proxied.rs
//! `proxied_route_cs`); and what the check against the eye's own walk
//! counts.

/// The cast's dispatches a route lays, three words each, in this order: the
/// cast as built, its beam rays, cells, samples, pixels and those they
/// missed; then the proxied cast's pixels and those they missed.
pub const BEAM: u32 = 0;
pub const CELLS: u32 = 1;
pub const SAMPLE: u32 = 2;
pub const CAST: u32 = 3;
pub const MISS: u32 = 4;
pub const PIXELS: u32 = 5;
pub const PIXELS_MISS: u32 = 6;
pub const ROUTES: u32 = 7;
pub const ROUTE_WORDS: u32 = 3 * ROUTES;

/// Whether route `r` is the proxied cast's: a frame runs those, or the
/// rest, never both.
pub const fn proxied(r: u32) -> bool {
    r >= PIXELS
}

/// What the check against the eye's own walk counts, a word each past the
/// oracle's (pxl-render gpu/ray/cast.rs `ORACLE_COUNTS`): the pixels the
/// proxied cast stopped, those it left to the sky, those of them the eye's
/// walk stops in (a hole in the boxes), the stops the eye's walk makes
/// otherwise, the eye's walks that lacked, the starts past where the eye's
/// walk stops (a box not conservative), the starts held over their box's
/// node, and the visits.
pub const STOPS: u32 = 0;
pub const SKY: u32 = 1;
pub const HOLES: u32 = 2;
pub const UNLIKE_EYE: u32 = 3;
pub const EYE_LACKED: u32 = 4;
pub const LATE: u32 = 5;
pub const HELD: u32 = 6;
pub const VISITS: u32 = 7;
pub const COUNTS: u32 = 8;

// pxl-spec/asset/star.rs

//! A star of the Yale Bright Star Catalog (Hoffleit and Warren 1991, CDS
//! V/50) as the sky's catalog holds it, written by the bake
//! (pxl-geo-bake bin/bake_star.rs) and read by the sky (pxl-render
//! gpu/star.rs): one layout, these two functions.
//!
//! A record is `RECORD_BYTES` little-endian: the right ascension a u16 of a
//! turn, the declination an i16 of a quarter turn, the visual magnitude and
//! B−V i16 thousandths, each rounded half to even. The catalog runs
//! brightest first, so a prefix of it is the sky to a magnitude.

pub const RECORD_BYTES: usize = 8;

/// A star as the catalog gives it: right ascension and declination in
/// degrees, visual magnitude, and B−V.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Star {
    pub ra: f64,
    pub dec: f64,
    pub vmag: f64,
    pub bv: f64,
}

/// `star`'s record.
pub fn record(star: &Star) -> [u8; RECORD_BYTES] {
    let ra = (libm::rint(star.ra / 360.0 * 65536.0) as u32 % 65536) as u16;
    let dec = libm::rint(star.dec / 90.0 * 32767.0) as i16;
    let [vmag, bv] = [star.vmag, star.bv].map(|v| libm::rint(v * 1000.0) as i16);
    let mut r = [0; RECORD_BYTES];
    r[0..2].copy_from_slice(&ra.to_le_bytes());
    r[2..4].copy_from_slice(&dec.to_le_bytes());
    r[4..6].copy_from_slice(&vmag.to_le_bytes());
    r[6..8].copy_from_slice(&bv.to_le_bytes());
    r
}

/// The star a record holds (`record`).
pub fn star(r: &[u8; RECORD_BYTES]) -> Star {
    let word = |k: usize| [r[2 * k], r[2 * k + 1]];
    Star {
        ra: f64::from(u16::from_le_bytes(word(0))) / 65536.0 * 360.0,
        dec: f64::from(i16::from_le_bytes(word(1))) / 32767.0 * 90.0,
        vmag: f64::from(i16::from_le_bytes(word(2))) / 1000.0,
        bv: f64::from(i16::from_le_bytes(word(3))) / 1000.0,
    }
}

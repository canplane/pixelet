// pxl-render/gpu/stars.test.rs

use super::*;

/// The catalog's own entries for its three brightest stars (CDS V/50, the
/// ReadMe's J2000 columns): right ascension and declination in degrees,
/// visual magnitude.
const BRIGHTEST: [(&str, f64, f64, f64); 3] = [
    ("Sirius", (6.0 + 45.0 / 60.0 + 8.9 / 3600.0) * 15.0, -(16.0 + 42.0 / 60.0 + 58.0 / 3600.0), -1.46),
    ("Canopus", (6.0 + 23.0 / 60.0 + 57.1 / 3600.0) * 15.0, -(52.0 + 41.0 / 60.0 + 44.0 / 3600.0), -0.72),
    ("Arcturus", (14.0 + 15.0 / 60.0 + 39.7 / 3600.0) * 15.0, 19.0 + 10.0 / 60.0 + 57.0 / 3600.0, -0.04),
];

/// The baked file is written by script/bake-stars.ts and read here, two
/// spellings of one layout. Read through `star`, the file's first stars have
/// to be where and as bright as the catalog says, so a bake that changed
/// the layout without this reader, or the other way round, fails here rather
/// than drawing the sky scrambled. Within the bake's quantization: 20″ in
/// right ascension, 10″ in declination, a thousandth of a magnitude.
#[test]
fn the_baked_catalog_reads_back_as_the_catalog_s_brightest_stars() {
    let records = CATALOG.as_chunks::<STAR_BYTES>().0;
    for (packed, (name, ra, dec, vmag)) in records.iter().zip(BRIGHTEST) {
        let s = star(packed);
        let (ra, dec) = (ra.to_radians(), dec.to_radians());
        let want = [dec.cos() * ra.cos(), dec.cos() * ra.sin(), dec.sin()];
        // The chord between the two directions rather than the acos of their
        // dot: a direction in f32 leaves a dot near one with too few bits to
        // tell arcseconds apart.
        let chord = (0..3).map(|k| (s[k] as f64 - want[k]).powi(2)).sum::<f64>().sqrt();
        let off = (2.0 * (chord / 2.0).asin()).to_degrees() * 3600.0;
        assert!(off < 30.0, "{name} reads {off:.1}″ from where the catalog puts it");
        let read = -2.5 * (s[3] as f64).log10();
        assert!((read - vmag).abs() < 2e-3, "{name} reads magnitude {read:.3}, the catalog says {vmag}");
    }
}

/// Brightest first, so that a prefix of the file is the sky to a magnitude.
#[test]
fn the_baked_catalog_runs_brightest_first() {
    let flux: Vec<f32> = CATALOG.as_chunks::<STAR_BYTES>().0.iter().map(|p| star(p)[3]).collect();
    assert!(flux.windows(2).all(|w| w[0] >= w[1]), "a star brighter than the one before it");
}

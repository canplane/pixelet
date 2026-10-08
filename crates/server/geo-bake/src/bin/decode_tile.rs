// pxl-geo-bake/bin/decode_tile.rs

//! A baked terrain tile read back the way the kernel reads it, checked
//! against the bake's contract. A byte that is no look draws nothing, so
//! one is a fault unless it is the unknown past the coverage. Run `--all` after
//! `deno task bake:tiles`; one tile also writes PNGs for the eye.
//!
//!   decode_tile 12720250262323200                # one leaf: summary + PNGs
//!   decode_tile 3179787687624704 --z 14          # a tile at a coarser zoom
//!   decode_tile 12720250262323200 --out /tmp/tile
//!   decode_tile out/terrain/v8/z15/12720250262323200  # a path works too
//!   decode_tile --all                            # every tile of every zoom, checked
//!   decode_tile --all --version v7

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use flate2::write::ZlibEncoder;
use flate2::{Compression, Crc};
use pxl_geo_bake::cli::Flags;
use pxl_geo_bake::coverage::Coverage;
use pxl_geo_bake::util::{encoding_of, unpack};
use pxl_spec::bake::constant::ELE_PADDED_SPAN;
use pxl_spec::bake::deck::{Decks, Form};
use pxl_spec::bake::format::decode_terrain;
use pxl_spec::bake::format::{GroundCell, Occupancy};
use pxl_spec::bake::published::PUBLISHED_VERSION;
use pxl_spec::bake::surface::bicubic;
use pxl_spec::bake::surface::extent_of;
use pxl_spec::cell::color;
use pxl_spec::cell::cube_types::{ground, sentinel, Ground};
use pxl_spec::space::coord::zoom::idx;
use pxl_spec::space::layer::lattice::{GROUND, TILE_CELLS};
use pxl_spec::space::layer::tile::{Tile as Placed, LEAF_ZOOM};
use xpute_core::math::scalar::round;
use xpute_core::wire::xtp::{HDR_SZ, WORD_SZ};

const MAGENTA: u32 = 0xff00ff;

/// Packed as a bake stores it, or identity from a deployment.
fn read_tile(path: &Path) -> Result<Vec<u8>, String> {
    let raw = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    match encoding_of(&raw) {
        Some(_) => unpack(&raw).map_err(|e| e.to_string()),
        None => Ok(raw),
    }
}

struct Tile {
    ele: Vec<i16>,
    cells: Vec<GroundCell>,
    extent: Vec<[i16; 2]>,
    occupancy: Occupancy,
    decks: Decks,
}

fn decode(pkt: &[u8]) -> Result<Tile, String> {
    let t = decode_terrain(pkt).map_err(|e| format!("not a tile: {e:?}"))?;
    let (ele, cells, extent, occupancy, decks) = (t.ele.into_owned(), t.ground.cells.into_owned(), t.index.extent().to_vec(), t.index.occupancy, t.decks);
    // Nothing may follow the tree: the decoder only checks the payload is there.
    let at = WORD_SZ as usize;
    let payload = u32::from_le_bytes(pkt[at..at + 4].try_into().expect("a header word")) as usize;
    if payload + HDR_SZ as usize != pkt.len() {
        return Err(format!("packet is {} bytes, header says {}", pkt.len(), payload + HDR_SZ as usize));
    }
    Ok(Tile { ele, cells, extent, occupancy, decks })
}

fn name_of(cell: GroundCell) -> &'static str {
    ground::ENTRIES.iter().find(|(_, n)| *n == cell).map_or("?", |(name, _)| name)
}

struct Summary {
    min: i16,
    max: i16,
    mean: f64,
    extent: [i16; 2],
    occupancy: Occupancy,
    counts: BTreeMap<GroundCell, u32>,
    faults: Vec<String>,
}

/// `at`: where the tile lies, which says which of its cells are past the coverage.
fn summarize(tile: &Tile, at: Option<Placed>) -> Summary {
    let (min, max) = tile.ele.iter().fold((i16::MAX, i16::MIN), |(lo, hi), &h| (lo.min(h), hi.max(h)));
    let mean = tile.ele.iter().map(|&h| h as f64).sum::<f64>() / tile.ele.len() as f64;
    let mut counts = BTreeMap::new();
    for &cell in &tile.cells {
        *counts.entry(cell).or_insert(0) += 1;
    }
    let mut faults = Vec::new();
    let coverage = Coverage::of_source().ok();
    let past_coverage = |i: usize| match (at, &coverage) {
        (Some(p), Some(c)) => !c.contains(idx(LEAF_ZOOM, GROUND.cell_at(p, i as u32).base().i)),
        _ => false,
    };
    let mut stray = BTreeMap::new();
    for (i, &cell) in tile.cells.iter().enumerate() {
        if Ground::of(cell).is_none() && !(cell == sentinel::unknown && past_coverage(i)) {
            *stray.entry(cell).or_insert(0u32) += 1;
        }
    }
    let past: Vec<String> = stray.iter().map(|(c, n)| format!("{c}×{n}")).collect();
    if !past.is_empty() {
        faults.push(format!("byte(s) that are no look — {}", past.join(", ")));
    }
    let short = extent_of(&tile.ele, &tile.decks).iter().zip(&tile.extent).filter(|([lo, hi], [ilo, ihi])| ilo > lo || ihi < hi).count();
    if short > 0 {
        faults.push(format!("{short} index cell(s) narrower than the tile's own surface"));
    }
    let extent = tile.extent.iter().fold([i16::MAX, i16::MIN], |[lo, hi], &[l, h]| [lo.min(l), hi.max(h)]);
    Summary {
        min,
        max,
        mean,
        extent,
        occupancy: tile.occupancy,
        counts,
        faults,
    }
}

fn print_summary(name: &str, s: &Summary) {
    println!("tile {name}");
    println!("  ele  min {} max {} mean {:.2}{}", s.min, s.max, s.mean, if s.min == s.max { " (flat)" } else { "" });
    println!("  index  lowest {} highest {} occupancy {:#07b}", s.extent[0], s.extent[1], s.occupancy.bits());
    let list: Vec<String> = s.counts.iter().map(|(&c, n)| format!("{} {n}", name_of(c))).collect();
    println!("  ground    {}", list.join(", "));
}

fn print_bores(tile: &Tile) {
    let bores: Vec<_> = tile.decks.iter().filter(|d| d.form == Form::Bore).collect();
    if bores.is_empty() {
        return;
    }
    let (mut under, mut points) = (0, 0);
    for d in &bores {
        for i in 0..d.points.len() {
            let [x, y] = d.point(i);
            points += 1;
            if i32::from(d.tops[i]) < i32::from(bicubic(&tile.ele, x, y)) {
                under += 1;
            }
        }
    }
    println!("  bores     {}, {points} points, {under} under the surface", bores.len());
}

fn print_decks(decks: &Decks) {
    let tops = decks.tops.iter().fold([i16::MAX, i16::MIN], |[lo, hi], &t| [lo.min(t), hi.max(t)]);
    match decks.len() {
        0 => println!("  decks     none"),
        n => println!("  decks     {n}, {} points, tops {}..{}", decks.points.len(), tops[0], tops[1]),
    }
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut crc = Crc::new();
    crc.update(kind);
    crc.update(data);
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc.sum().to_be_bytes());
}

fn png(width: u32, height: u32, channels: u8, pixels: &[u8]) -> Vec<u8> {
    let stride = (width * channels as u32) as usize;
    let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
    for row in pixels.chunks(stride) {
        z.write_all(&[0]).and_then(|_| z.write_all(row)).expect("a write into memory");
    }
    let data = z.finish().expect("a write into memory");
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, if channels == 1 { 0 } else { 2 }, 0, 0, 0]);
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &data);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn heights_png(tile: &Tile, s: &Summary) -> Vec<u8> {
    let span = (s.max as f64 - s.min as f64).max(0.0);
    let gray: Vec<u8> = tile
        .ele
        .iter()
        .map(|&h| if span == 0.0 { 128 } else { round((h as f64 - s.min as f64) / span * 255.0) as u8 })
        .collect();
    png(ELE_PADDED_SPAN, ELE_PADDED_SPAN, 1, &gray)
}

fn rgb_of(cell: GroundCell) -> u32 {
    color::ground(cell).map_or(MAGENTA, |s| s.rgb)
}

fn ground_png(tile: &Tile) -> Vec<u8> {
    let px: Vec<u8> = tile
        .cells
        .iter()
        .flat_map(|&c| {
            let v = rgb_of(c);
            [(v >> 16) as u8, (v >> 8) as u8, v as u8]
        })
        .collect();
    png(TILE_CELLS, TILE_CELLS, 3, &px)
}

fn decks_png(tile: &Tile) -> Vec<u8> {
    let n = TILE_CELLS as i64;
    let mut top = vec![f64::NEG_INFINITY; (n * n) as usize];
    let mut px = vec![0u8; (n * n * 3) as usize];
    tile.decks.over_grid(None, [0.0, 0.0], 1.0, [n, n], |x, y, _, over| {
        let i = (y * n + x) as usize;
        if over.top < top[i] {
            return;
        }
        top[i] = over.top;
        let v = rgb_of(over.kind);
        px[i * 3..i * 3 + 3].copy_from_slice(&[(v >> 16) as u8, (v >> 8) as u8, v as u8]);
    });
    png(TILE_CELLS, TILE_CELLS, 3, &px)
}

fn stored_tiles(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            stored_tiles(&path, out)?;
        // `_built`, `_snapshot`: a version's records about itself, no tile.
        } else if !matches!(path.extension().and_then(|e| e.to_str()), Some("part" | "empty")) && !path.file_name().is_some_and(|n| n.to_string_lossy().starts_with('_')) {
            out.push(path);
        }
    }
    Ok(())
}

fn check_all(tiles: &Path) -> ExitCode {
    let mut paths = Vec::new();
    if let Err(e) = stored_tiles(tiles, &mut paths) {
        eprintln!("[decode_tile] {}: {e} — bake first, or name the version", tiles.display());
        return ExitCode::FAILURE;
    }
    if paths.is_empty() {
        eprintln!("[decode_tile] no tiles under {} — bake first, or name the version", tiles.display());
        return ExitCode::FAILURE;
    }
    let mut bad = Vec::new();
    for (k, path) in paths.iter().enumerate() {
        let name = path.strip_prefix(tiles).unwrap_or(path).display().to_string();
        match read_tile(path).and_then(|pkt| decode(&pkt)) {
            Ok(tile) => {
                let s = summarize(&tile, path.file_name().and_then(|n| Placed::of_name(&n.to_string_lossy())));
                if !s.faults.is_empty() {
                    bad.push(format!("{name}: {}", s.faults.join("; ")));
                }
            }
            Err(e) => bad.push(format!("{name}: {e}")),
        }
        if (k + 1) % 1000 == 0 {
            eprintln!("[decode_tile] {} tiles read, {} at fault", k + 1, bad.len());
        }
    }
    for line in &bad {
        eprintln!("  {line}");
    }
    if !bad.is_empty() {
        eprintln!("[decode_tile] {} of {} tile(s) under {} at fault", bad.len(), paths.len(), tiles.display());
        return ExitCode::FAILURE;
    }
    println!("[decode_tile] ok — {} tile(s) under {} read as the kernel reads them", paths.len(), tiles.display());
    ExitCode::SUCCESS
}

fn one(path: &Path, out: Option<PathBuf>) -> Result<bool, String> {
    let tile = decode(&read_tile(path)?)?;
    let s = summarize(&tile, path.file_name().and_then(|n| Placed::of_name(&n.to_string_lossy())));
    let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    print_summary(&name, &s);
    print_decks(&tile.decks);
    print_bores(&tile);
    for line in &s.faults {
        eprintln!("[decode_tile] {line}");
    }
    let dir = out.unwrap_or_else(|| Path::new("cache/decoded").join(&name));
    let write = |file: String, bytes: Vec<u8>| std::fs::write(dir.join(&file), bytes).map_err(|e| format!("{file}: {e}"));
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    write("heights.png".into(), heights_png(&tile, &s))?;
    write("decks.png".into(), decks_png(&tile))?;
    write("ground.png".into(), ground_png(&tile))?;
    println!("[decode_tile] wrote heights.png, ground.png and decks.png under {}", dir.display());
    Ok(s.faults.is_empty())
}

fn main() -> ExitCode {
    const USAGE: &str = "usage: decode_tile <key | path> [--z n] [--version v6] [--out dir] | --all [--version v6]";
    let (mut version, mut z, mut out, mut all, mut target) = (PUBLISHED_VERSION.terrain.to_string(), "16".to_string(), None, false, None);
    let mut flags = Flags::from_env();
    let read = (|| {
        while let Some(arg) = flags.arg() {
            match arg.as_str() {
                "--version" => version = flags.value()?,
                "--z" => z = flags.value()?,
                "--out" => out = Some(PathBuf::from(flags.value()?)),
                "--all" => all = true,
                _ if target.is_none() && !arg.starts_with("--") => target = Some(arg),
                other => return Err(format!("unknown argument: {other}")),
            }
        }
        Ok::<(), String>(())
    })();
    if let Err(e) = read {
        eprintln!("{e}\n{USAGE}");
        return ExitCode::from(2);
    }
    let tiles = Path::new("out/terrain").join(&version);
    if all {
        return check_all(&tiles);
    }
    let Some(target) = target else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let path = if target.bytes().all(|b| b.is_ascii_digit()) {
        tiles.join(format!("z{z}")).join(target)
    } else {
        PathBuf::from(target)
    };
    match one(&path, out) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("[decode_tile] {}: {e}", path.display());
            ExitCode::FAILURE
        }
    }
}
